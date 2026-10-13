// Native runtime: the typed lists of the VM's `typed_data_patch.dart`.
//
// The representation is the unified space's (`tipadas.rs`,
// docs/NATIVO-ESPACO-UNIFICADO.md §2.5): the internal list (`_Uint8List`,
// `_Float64List`, …; cids 22–35) is a `BRUTO` block with the length, the address
// of the bytes and the bytes (or only the address of outside memory, with
// `EXTERNAL`); the view (`_Uint8ArrayView`, `_ByteDataView`, …; cids 36–65) is a
// four-field object with the length and the address in the same places, the
// internal list and the offset. The bytes are in host endianness. The
// `vm:recognized` members and the VM's `TypedData*` natives that the patch's Dart
// uses are the functions here (table in `crates/emit_native/src/nativos.rs`):
// factories, `length`, `offsetInBytes`, `_typedData`, `_getX`/`_setX`, `[]`,
// `_memMoveN` and `_setClampedRange`. Generated code reads the length and the data
// inline (`llvm/tipados_ir.rs`), and these functions remain as the slow path (with
// the VM's errors).

/// The element types (the `tipo` of the heap's shapes; `tipadas::tipo`).
const TYPE_INT8: u8 = crate::tipadas::tipo::INT8;
const TYPE_UINT8: u8 = crate::tipadas::tipo::UINT8;
const TYPE_UINT8_CLAMPED: u8 = crate::tipadas::tipo::UINT8_CLAMPED;
const TYPE_INT16: u8 = crate::tipadas::tipo::INT16;
const TYPE_UINT16: u8 = crate::tipadas::tipo::UINT16;
const TYPE_INT32: u8 = crate::tipadas::tipo::INT32;
const TYPE_UINT32: u8 = crate::tipadas::tipo::UINT32;
const TYPE_INT64: u8 = crate::tipadas::tipo::INT64;
const TYPE_UINT64: u8 = crate::tipadas::tipo::UINT64;
const TYPE_FLOAT32: u8 = crate::tipadas::tipo::FLOAT32;
const TYPE_FLOAT64: u8 = crate::tipadas::tipo::FLOAT64;
const TYPE_FLOAT32X4: u8 = crate::tipadas::tipo::FLOAT32X4;
const TYPE_INT32X4: u8 = crate::tipadas::tipo::INT32X4;
const TYPE_FLOAT64X2: u8 = crate::tipadas::tipo::FLOAT64X2;
const TYPE_BYTE_DATA: u8 = crate::tipadas::tipo::BYTE_DATA;

/// The bit of [`dartforge_view_new`]'s `tipo` that marks the view as unmodifiable
/// (`_UnmodifiableXArrayView`).
const UNMODIFIABLE_VIEW: i64 = 0x100;

/// Bytes per element of a type.
fn tamanho_do_elemento(tipo: u8) -> usize {
    crate::tipadas::tamanho_do_elemento(tipo)
}

/// The view of `h` (internal list, external list or view), or `None`.
fn resolver(heap: &Heap, h: i64) -> Option<crate::tipadas::TypedRef> {
    heap.tipada(h)
}

/// The view of `h` without holding the heap borrow (the address and the size stay
/// valid while the list lives: the collector does not move).
fn typed_view(h: i64) -> Option<crate::tipadas::TypedRef> {
    HEAP.with(|heap| heap.borrow().tipada(h))
}

/// Throws `RangeError.range(value, 0, max, "length")` — what the VM throws for an
/// invalid length when creating a typed list.
fn throw_length(value: i64, max: i64) {
    throw_range_err(value, 0, max, "length");
}

/// The largest length the VM accepts at creation (the maximum of a `Smi`); beyond
/// what the heap holds, the heap ceiling terminates with the memory message, like
/// the VM with `OutOfMemoryError`.
fn max_elements(_type: u8) -> i64 {
    (1i64 << 62) - 1
}

/// Allocates a zeroed internal typed list of `n` elements. `class_id` is the fixed
/// cid of the list of `tipo` (`layout::cid::tipada`), which the emitter passes to
/// register the method table (`_t`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_new(class_id: i64, tipo: i64, n: i64) -> i64 {
    let tipo = tipo as u8;
    debug_assert_eq!(class_id, i64::from(crate::layout::cid::tipada(tipo)), "typed list cid outside the contract");
    let max = max_elements(tipo);
    if !(0..=max).contains(&n) {
        throw_length(n, max);
        return 0;
    }
    HEAP.with(|h| h.borrow_mut().new_typed(tipo, n as usize))
}

/// [`dartforge_typed_new`] that registers the class's method table on the first
/// allocation (like `dartforge_object_new_t`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_new_t(class_id: i64, tipo: i64, n: i64, f: extern "C" fn() -> *const i64) -> i64 {
    dartforge_register_table(class_id, f);
    dartforge_typed_new(class_id, tipo, n)
}

/// Allocates a view over `base` (an internal list; a view passed here is resolved
/// to its internal list). The [`UNMODIFIABLE_VIEW`] bit of `tipo` marks the view as
/// unmodifiable; the type [`TYPE_BYTE_DATA`] is `_ByteDataView`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_view_new(class_id: i64, tipo: i64, base: i64, offset: i64, length: i64) -> i64 {
    let cid = crate::tipadas::view_cid((tipo & 0xFF) as u8, tipo & UNMODIFIABLE_VIEW != 0);
    debug_assert_eq!(class_id, i64::from(cid), "view cid outside the contract");
    // `new_view` roots the base during the allocation.
    HEAP.with(|h| h.borrow_mut().new_view(cid, base, offset.max(0) as usize, length.max(0) as usize))
}

/// [`dartforge_view_new`] that registers the class's method table.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_view_new_t(
    class_id: i64,
    tipo: i64,
    base: i64,
    offset: i64,
    length: i64,
    f: extern "C" fn() -> *const i64,
) -> i64 {
    dartforge_register_table(class_id, f);
    dartforge_view_new(class_id, tipo, base, offset, length)
}

/// `TypedDataBase_length`: the length in elements.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedDataBase_length(this: i64) -> i64 {
    typed_view(this).map_or(0, |t| t.len as i64)
}

/// The length in elements of `h` if it is a typed list of the given `tipo`
/// (internal, external or view) and, for `escrita != 0`, modifiable; otherwise 0.
/// Slow path: generated code reads the length inline (`@df.typed_len`,
/// `llvm/tipados_ir.rs`); this remains for filling ([`dartforge_typed_fill_int`])
/// and until the cut (§3.7).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_len(h: i64, tipo: i64, write_side: i64) -> i64 {
    heap_without_borrow(|heap| match resolver(heap, h) {
        Some(t) if i64::from(t.tipo) == tipo && (write_side == 0 || !t.unmodifiable) => t.len as i64,
        _ => 0,
    })
}

/// The address of the first element of `h` if it is a typed list, otherwise 0
/// (without effect or error). The bytes do not move while the list lives (the
/// collector does not move), nor does the external memory of `asTypedList`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_ptr(h: i64) -> i64 {
    heap_without_borrow(|heap| resolver(heap, h).map_or(0, |t| t.data as i64))
}

/// `list.fillRange(start, end, value)` of a typed list of integers (an internal
/// list or a modifiable view of type `tipo`, except `Uint8ClampedList`), writing
/// the low bits of `value` like the VM's `[]=`. Returns 1 if it filled; 0 if it is
/// not the simple case (an empty or unsuitable list, an invalid range) — generated
/// code then calls the SDK's `fillRange`, which throws the VM's error. Before, the
/// SDK's `fillRange` wrote element by element through dispatch (~30 ns each; the
/// `clear` of a 512×512 frame cost 26 ms).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_fill_int(h: i64, tipo: i64, start: i64, end: i64, value: i64) -> i64 {
    let Some((p, a, b)) = range_to_fill(h, tipo, start, end) else { return 0 };
    // SAFETY: `p` is the first element of a suitable list of `n ≥ end` elements
    // of type `tipo`, with no assumed alignment.
    unsafe {
        match tipo as u8 {
            TYPE_INT8 | TYPE_UINT8 => std::ptr::write_bytes(p.add(a), value as u8, b - a),
            TYPE_INT16 | TYPE_UINT16 => (a..b).for_each(|i| (p as *mut u16).add(i).write_unaligned(value as u16)),
            TYPE_INT32 | TYPE_UINT32 => (a..b).for_each(|i| (p as *mut u32).add(i).write_unaligned(value as u32)),
            TYPE_INT64 | TYPE_UINT64 => (a..b).for_each(|i| (p as *mut u64).add(i).write_unaligned(value as u64)),
            _ => return 0,
        }
    }
    1
}

/// [`dartforge_typed_fill_int`] for `Float32List`/`Float64List`: the `double`
/// rounded to `float` in `Float32List`, like `[]=`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_fill_double(h: i64, tipo: i64, start: i64, end: i64, value: f64) -> i64 {
    let Some((p, a, b)) = range_to_fill(h, tipo, start, end) else { return 0 };
    // SAFETY: as in `dartforge_typed_fill_int`.
    unsafe {
        match tipo as u8 {
            TYPE_FLOAT32 => (a..b).for_each(|i| (p as *mut f32).add(i).write_unaligned(value as f32)),
            TYPE_FLOAT64 => (a..b).for_each(|i| (p as *mut f64).add(i).write_unaligned(value)),
            _ => return 0,
        }
    }
    1
}

/// The address of the first element and the range `start..end` of a list of type
/// `tipo` suitable for writing, if the range is valid and not empty.
fn range_to_fill(h: i64, tipo: i64, start: i64, end: i64) -> Option<(*mut u8, usize, usize)> {
    if tipo == i64::from(TYPE_UINT8_CLAMPED) {
        return None;
    }
    let n = dartforge_typed_len(h, tipo, 1);
    if n == 0 || start < 0 || end <= start || end > n {
        return None;
    }
    let p = dartforge_typed_ptr(h) as *mut u8;
    (!p.is_null()).then_some((p, start as usize, end as usize))
}

/// `TypedDataView_typedData`: the internal list of a view.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedDataView_typedData(this: i64) -> i64 {
    typed_view(this).map_or(0, |t| t.base.unwrap_or(this))
}

/// `TypedDataView_offsetInBytes`: the offset of a view.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedDataView_offsetInBytes(this: i64) -> i64 {
    typed_view(this).map_or(0, |t| t.offset as i64)
}

/// The address of the `N` bytes of `this` at `off`, if they fit; otherwise throws
/// the VM's `RangeError` (`IndexError` over the bytes) and returns `None`.
fn bytes_address<const N: usize>(this: i64, off: i64) -> Option<*mut u8> {
    let t = typed_view(this);
    let inside = t.and_then(|t| {
        let off = usize::try_from(off).ok()?;
        let end = off.checked_add(N)?;
        (end <= t.bytes).then(|| t.data.wrapping_add(off))
    });
    if inside.is_none() {
        throw_index(off, this, t.map_or(0, |t| t.bytes as i64));
    }
    inside
}

/// Reads `N` bytes of list `this` at `off`; out of bounds it throws `RangeError`
/// and returns `None`.
fn do_read<const N: usize>(this: i64, off: i64) -> Option<[u8; N]> {
    let p = bytes_address::<N>(this, off)?;
    let mut b = [0u8; N];
    // SAFETY: `p..p+N` is within the bytes of the live list (checked).
    unsafe { std::ptr::copy_nonoverlapping(p, b.as_mut_ptr(), N) };
    Some(b)
}

/// Writes `v` into list `this` at `off`; out of bounds it throws.
fn write_value<const N: usize>(this: i64, off: i64, v: [u8; N]) {
    if let Some(p) = bytes_address::<N>(this, off) {
        // SAFETY: as in `do_read`.
        unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), p, N) };
    }
}

/// `_getInt8`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_getInt8(this: i64, off: i64) -> i64 {
    do_read::<1>(this, off).map_or(0, |b| i8::from_ne_bytes(b) as i64)
}

/// `_setInt8`: the value truncated to the element type.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_setInt8(this: i64, off: i64, v: i64) {
    write_value::<1>(this, off, (v as i8).to_ne_bytes());
}

/// `_getUint8`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_getUint8(this: i64, off: i64) -> i64 {
    do_read::<1>(this, off).map_or(0, |b| u8::from_ne_bytes(b) as i64)
}

/// `_setUint8`: the value truncated to the element type.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_setUint8(this: i64, off: i64, v: i64) {
    write_value::<1>(this, off, (v as u8).to_ne_bytes());
}

/// `_getInt16`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_getInt16(this: i64, off: i64) -> i64 {
    do_read::<2>(this, off).map_or(0, |b| i16::from_ne_bytes(b) as i64)
}

/// `_setInt16`: the value truncated to the element type.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_setInt16(this: i64, off: i64, v: i64) {
    write_value::<2>(this, off, (v as i16).to_ne_bytes());
}

/// `_getUint16`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_getUint16(this: i64, off: i64) -> i64 {
    do_read::<2>(this, off).map_or(0, |b| u16::from_ne_bytes(b) as i64)
}

/// `_setUint16`: the value truncated to the element type.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_setUint16(this: i64, off: i64, v: i64) {
    write_value::<2>(this, off, (v as u16).to_ne_bytes());
}

/// `_getInt32`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_getInt32(this: i64, off: i64) -> i64 {
    do_read::<4>(this, off).map_or(0, |b| i32::from_ne_bytes(b) as i64)
}

/// `_setInt32`: the value truncated to the element type.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_setInt32(this: i64, off: i64, v: i64) {
    write_value::<4>(this, off, (v as i32).to_ne_bytes());
}

/// `_getUint32`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_getUint32(this: i64, off: i64) -> i64 {
    do_read::<4>(this, off).map_or(0, |b| u32::from_ne_bytes(b) as i64)
}

/// `_setUint32`: the value truncated to the element type.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_setUint32(this: i64, off: i64, v: i64) {
    write_value::<4>(this, off, (v as u32).to_ne_bytes());
}

/// `_getInt64`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_getInt64(this: i64, off: i64) -> i64 {
    do_read::<8>(this, off).map_or(0, i64::from_ne_bytes)
}

/// `_setInt64`: the value truncated to the element type.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_setInt64(this: i64, off: i64, v: i64) {
    write_value::<8>(this, off, v.to_ne_bytes());
}

/// `_getUint64`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_getUint64(this: i64, off: i64) -> i64 {
    do_read::<8>(this, off).map_or(0, |b| u64::from_ne_bytes(b) as i64)
}

/// `_setUint64`: the value truncated to the element type.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_setUint64(this: i64, off: i64, v: i64) {
    write_value::<8>(this, off, (v as u64).to_ne_bytes());
}

/// `TypedData_GetFloat32` (`_getFloat32`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedData_GetFloat32(this: i64, off: i64) -> f64 {
    do_read::<4>(this, off).map_or(0.0, |b| f64::from(f32::from_ne_bytes(b)))
}

/// `TypedData_SetFloat32` (`_setFloat32`): the `double` rounded to `float`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedData_SetFloat32(this: i64, off: i64, v: f64) {
    write_value::<4>(this, off, (v as f32).to_ne_bytes());
}

/// `TypedData_GetFloat64` (`_getFloat64`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedData_GetFloat64(this: i64, off: i64) -> f64 {
    do_read::<8>(this, off).map_or(0.0, f64::from_ne_bytes)
}

/// `TypedData_SetFloat64` (`_setFloat64`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedData_SetFloat64(this: i64, off: i64, v: f64) {
    write_value::<8>(this, off, v.to_ne_bytes());
}

/// Element `i` of a list or view (the VM's `[]`), checked against the length:
/// `RangeError.range(i, 0, n - 1, "length")` outside it. Returns the view and the
/// element's address.
fn element(this: i64, i: i64) -> Option<(crate::tipadas::TypedRef, *mut u8)> {
    let t = typed_view(this)?;
    if i < 0 || i as usize >= t.len {
        throw_range_err(i, 0, t.len as i64 - 1, "length");
        return None;
    }
    Some((t, t.data.wrapping_add(i as usize * tamanho_do_elemento(t.tipo))))
}

/// Element `i` (integer bits) of a list or view of integers, with the VM's index
/// check.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_index_int(this: i64, i: i64) -> i64 {
    let Some((t, p)) = element(this, i) else { return 0 };
    // SAFETY: `p` is element `i < len` of the live list, with no assumed alignment.
    unsafe {
        match t.tipo {
            TYPE_INT8 => i64::from(p.cast::<i8>().read()),
            TYPE_UINT8 | TYPE_UINT8_CLAMPED | TYPE_BYTE_DATA => i64::from(p.read()),
            TYPE_INT16 => i64::from(p.cast::<i16>().read_unaligned()),
            TYPE_UINT16 => i64::from(p.cast::<u16>().read_unaligned()),
            TYPE_INT32 => i64::from(p.cast::<i32>().read_unaligned()),
            TYPE_UINT32 => i64::from(p.cast::<u32>().read_unaligned()),
            TYPE_INT64 | TYPE_UINT64 => p.cast::<i64>().read_unaligned(),
            _ => 0,
        }
    }
}

/// Element `i` of a list or view of `double`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_index_double(this: i64, i: i64) -> f64 {
    let Some((t, p)) = element(this, i) else { return 0.0 };
    // SAFETY: as in `dartforge_native_DartForge_typed_index_int`.
    unsafe {
        match t.tipo {
            TYPE_FLOAT32 => f64::from(p.cast::<f32>().read_unaligned()),
            _ => p.cast::<f64>().read_unaligned(),
        }
    }
}

/// `_memMoveN(start, count, from, skipCount)`: copies `count` elements of `N`
/// bytes from `from` (starting at `skipCount`) to `this` (starting at `start`),
/// like `memmove` — both may be the same memory. The patch's Dart checks the
/// ranges first; here we only avoid going past the bytes of each one.
fn relocate(this: i64, start: i64, count: i64, from: i64, skip: i64, n: usize) {
    let (Some(d), Some(s)) = (typed_view(this), typed_view(from)) else { return };
    let bytes = count.max(0) as usize * n;
    let de = skip.max(0) as usize * n;
    let to = start.max(0) as usize * n;
    if de + bytes > s.bytes || to + bytes > d.bytes || bytes == 0 {
        return;
    }
    // SAFETY: both ranges are within the live lists; `copy` is `memmove` (the
    // ranges may overlap).
    unsafe { std::ptr::copy(s.data.add(de), d.data.add(to), bytes) };
}

/// `_memMove1`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_memMove1(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    relocate(this, start, count, from, skip, 1);
}

/// `_memMove2`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_memMove2(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    relocate(this, start, count, from, skip, 2);
}

/// `_memMove4`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_memMove4(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    relocate(this, start, count, from, skip, 4);
}

/// `_memMove8`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_memMove8(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    relocate(this, start, count, from, skip, 8);
}

/// `_memMove16`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_memMove16(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    relocate(this, start, count, from, skip, 16);
}

/// `TypedDataBase_setClampedRange(start, count, from, skipCount)`: copies the
/// integer elements of `from` (a list or view of signed integers) into a byte list,
/// saturating to 0..255.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedDataBase_setClampedRange(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    for k in 0..count.max(0) {
        let v = dartforge_native_DartForge_typed_index_int(from, skip + k);
        if dartforge_exception_pending() != 0 {
            return;
        }
        // The destination is a byte list: element `start + k` is the byte.
        let Some(d) = typed_view(this) else { return };
        let i = start + k;
        if i < 0 || i as usize >= d.bytes {
            throw_index(i, this, d.bytes as i64);
            return;
        }
        // SAFETY: `i < bytes` of the live list.
        unsafe { d.data.add(i as usize).write(v.clamp(0, 255) as u8) };
    }
}
