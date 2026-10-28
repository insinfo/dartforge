// Native runtime: list natives of the SDK from source (P5c/P5d, δ) and the errors
// the runtime throws as Dart objects from source.
//
// `_List` (cid 8), `_ImmutableList` (9) and `_GrowableList` (10) are object-space
// blocks with fixed cids (docs/NATIVO-ESPACO-UNIFICADO.md §2.5): the
// `_GrowableList` keeps (length, `_List`) like the VM, and the `GrowableList_*`
// natives are the VM's operations on those two fields. The view is in `lists.rs`
// (`Heap::lista_*`); here are only the index check, the VM's errors and the
// reified type (`types.rs`). The class never changes after creation: fixing or
// making unmodifiable means copying (§2.16).
//
// The errors: the overlay's `dart:_internal` library
// (`native_sdk/internal/print_patch.dart`) has `_dartforge*` functions that build
// the source's error (`IndexError`, `RangeError`…); its registration hands the
// addresses to the runtime (`dartforge_register_helper`). Without the SDK from
// source, the error is the runtime's (`exceptions.rs`).

thread_local! {
    /// Dart functions that the runtime calls, by name (`_dartforgeIndexError`…).
    static HELPERS: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::default());
}

/// Registers a Dart function that the runtime calls by name.
///
/// # Safety
/// `name` points to `len` readable UTF-8 bytes; `f` is the function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_helper(name: *const u8, len: i64, f: usize) {
    // SAFETY: a module constant with `len` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(name, len as usize) };
    let name = String::from_utf8_lossy(bytes).into_owned();
    HELPERS.with(|a| a.borrow_mut().insert(name, f));
}

fn helper(name: &str) -> Option<usize> {
    HELPERS.with(|a| a.borrow().get(name).copied())
}

/// `IndexError` of `ix` in `target` of size `size` (the VM's `RangeError
/// (index)`).
fn throw_index(ix: i64, target: i64, size: i64) {
    if let Some(f) = helper("_dartforgeIndexError") {
        // SAFETY: registered by the `dart:_internal` library with this signature
        // (`int`, `Object?`, `int`) → `Object`.
        let g = {
            let dart_target: usize = f;
            move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(dart_target, a0, a1, a2) }
        };
        let error = with_roots(&[target], || g(ix, target, size));
        if dartforge_exception_pending() != 0 {
            return;
        }
        with_roots(&[error], || dartforge_exception_throw(error, 3));
        return;
    }
    let n = allocate_str("index");
    let err = with_roots(&[n], || dartforge_range_error_index(ix, size, n, 0));
    dartforge_exception_throw(err, 3);
}

/// The bounds check of `[]` (`name` "length") and of the lists' `_setIndexed`
/// ("index") in the VM: `RangeError.range(index, 0, size - 1, name)`, not
/// `IndexError` (e.g. "RangeError (length): Invalid value: Not in inclusive range
/// 0..2: 10").
fn throw_range(ix: i64, size: i64, name: &str) {
    let n = allocate_str(name);
    let error = with_roots(&[n], || dartforge_range_error_range(ix, 0, size - 1, n, 0));
    if dartforge_exception_pending() != 0 {
        return;
    }
    with_roots(&[error], || dartforge_exception_throw(error, 3));
}

/// The length of the core list `this` (N4 panic if it is not one).
fn list_len(this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().list_len(this) as i64)
}

/// The `RangeError` of `ix` outside `[0, n)` with the VM's name (`"index"` on
/// writes, `"length"` on reads); `true` if it threw.
fn outside_list(ix: i64, n: i64, name: &str) -> bool {
    if ix < 0 || ix >= n {
        throw_range(ix, n, name);
        return true;
    }
    false
}

/// Writes the reified type `L<E>` (`class`: the concrete cid) of `h` from the
/// native's type-argument tuple, with `h` rooted (the shape that `E` requires may
/// decompact, and decompacting allocates).
fn type_tuple_list(h: i64, tuple: i64, class: i32) {
    if let Some(ty) = tuple_list_type(tuple, Some(i64::from(class))) {
        with_roots(&[h], || set_list_type(h, ty + 1));
    }
}

/// Writes into `h` (`class`) the reified type of `origin`, with its `E`.
fn type_copied_list(h: i64, origin: i64, class: i32) {
    if let Some(ty) = with_roots(&[h], || copied_list_type(origin, Some(i64::from(class)))) {
        with_roots(&[h], || set_list_type(h, ty + 1));
    }
}

/// The value stack of the `_JsonListener` of the `convert_patch.dart` overlay
/// (docs/NATIVO-PLANO.md §13.2, item 4): a `_GrowableList` of `Object?` that only
/// the listener sees, in the general shape. The natives below read and write it
/// through the contract's offsets (`Data` of `natives_hash.rs`), without the `Heap`,
/// and return -1 without changing anything when it is not in that shape — Dart
/// then does the usual `add`, loop and `length =`.
///
/// `_JsonListener._dfPush(stack, v)`: `v` at the end of the stack, without the
/// `add` dispatch nor the `E` check (`Object?`). Returns the new length. May collect
/// (the storage growth).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_json_push(stack: i64, v: i64) -> i64 {
    if let Some(n) = push_raw(stack, v) {
        return n;
    }
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let serve = heap.is_list(stack)
            && heap.class(stack) == crate::layout::cid::GROWABLE_LIST
            && heap.list_shape(stack) == crate::lists::Element::General;
        if !serve {
            return -1;
        }
        heap.list_push(stack, crate::heap::Value::Ref(v));
        heap.list_len(stack) as i64
    })
}

/// The push with spare capacity: writes into the storage (with the element
/// barrier) and the length into field 0, without the `Heap`. `None` if it needs to
/// grow or the shape is not the simple one.
fn push_raw(stack: i64, v: i64) -> Option<i64> {
    use crate::layout::{cid, is_object, offsets};
    if !is_object(stack) {
        return None;
    }
    // SAFETY: live object (the listener's `_stack` field).
    if unsafe { raw_header(stack) }.class_id != cid::GROWABLE_LIST {
        return None;
    }
    // SAFETY: `_GrowableList`: length in field 0, storage in field 1.
    let (len, a) = unsafe {
        (
            raw_word(stack, offsets::LENGTH),
            raw_word(stack, offsets::GROWABLE_DATA),
        )
    };
    let d = Data::of(a, true)?;
    let n = usize::try_from(len).ok()?;
    if n >= d.len {
        return None;
    }
    d.write_value(n, v);
    // SAFETY: field 0 (the raw length, no barrier: it is not a reference).
    unsafe { *in_block(stack, offsets::LENGTH).cast::<i64>() = len + 1 };
    Some(len + 1)
}

/// The mark at `start - 1` and the stack (cut at the length), if `start` is the
/// position of an open container (`1 ≤ start ≤ length`) and the mark a `Smi`.
fn stack_mark(stack: i64, start: i64) -> Option<(Data, usize, i64)> {
    let p = Data::from_growable(stack)?;
    let start = usize::try_from(start)
        .ok()
        .filter(|&i| i >= 1 && i <= p.len)?;
    let m = p.do_read(start - 1);
    crate::layout::smi::is_smi(m).then(|| (p, start, crate::layout::smi::value(m)))
}

/// Cuts the stack at `until` (nulling the removed positions, so as not to retain
/// the values; null does not need a barrier).
fn cut_stack(stack: i64, p: Data, until: usize) {
    for i in until..p.len {
        // SAFETY: `i < len` of the stack's writable general storage.
        let old = unsafe { std::mem::replace(&mut *p.p.add(i), 0) };
        if arc_enabled() {
            arc_raw_write(p.h, old, 0);
        }
    }
    // SAFETY: field 0 of the `_GrowableList` (the raw length).
    unsafe { *in_block(stack, crate::layout::offsets::LENGTH).cast::<i64>() = until as i64 };
}

/// `_JsonListener._dfTruncate(stack, start)`: removes the values `stack[start..]`
/// and the mark at `start - 1`; returns the mark, or -1. Does not allocate.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_json_truncate(stack: i64, start: i64) -> i64 {
    let Some((p, start, mark)) = stack_mark(stack, start) else {
        return -1;
    };
    // The stack is writable (the listener's): the same check as `push_raw`.
    // SAFETY: the `_GrowableList`'s storage (field 1).
    if Data::of(
        unsafe { raw_word(stack, crate::layout::offsets::GROWABLE_DATA) },
        true,
    )
    .is_none()
    {
        return -1;
    }
    cut_stack(stack, p, start - 1);
    mark
}

/// `_JsonListener._dfMark(stack, start)`: the mark at `start - 1` (the open
/// container), changing nothing; -1 outside the simple shape. Does not allocate.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_json_mark(stack: i64, start: i64) -> i64 {
    stack_mark(stack, start).map_or(-1, |(_, _, m)| m)
}

/// `_JsonListener._dfCloseList(stack, start, model)`: the JSON list with the
/// values `stack[start..]`, in order — a new `_GrowableList` of the exact size,
/// with the reified type of `model` (the listener's `[]` of `List<dynamic>`, the
/// same type as the `[]` the VM's listener creates per list) —, and the stack
/// without those values nor the mark at `start - 1`. null, changing nothing,
/// outside the simple shape. May collect.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_json_close_list(
    stack: i64,
    start: i64,
    model: i64,
) -> i64 {
    use crate::layout::{cid, is_object, offsets};
    let Some((p, i0, _)) = stack_mark(stack, start) else {
        return 0;
    };
    // SAFETY: the `_GrowableList`'s storage (field 1).
    if Data::of(unsafe { raw_word(stack, offsets::GROWABLE_DATA) }, true).is_none() {
        return 0;
    }
    // SAFETY: live object.
    if !is_object(model) || unsafe { raw_header(model) }.class_id != cid::GROWABLE_LIST {
        return 0;
    }
    let n = p.len - i0;
    let list = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // The stack and the model rooted: the values are reached through the stack.
        let list = heap.list_with_roots(&[stack, model], |heap| {
            heap.new_growable(n, n, crate::lists::Element::General)
        });
        let ty = heap.metadata(model);
        heap.set_metadata(list, ty);
        if n > 0 {
            // The collector does not move: `p` stays valid. The storage is new and
            // nobody else sees it: raw copy, no barrier (like `list_reserve`).
            let a = heap.list_data(list);
            // SAFETY: `stack_mark` checked the source range; the freshly
            // allocated storage has `n` elements and cannot alias the stack.
            // No allocation or collection occurs while the pointers are used.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    p.p.add(i0),
                    heap.words_mut(a).as_mut_ptr().add(1),
                    n,
                );
            }
        }
        list
    });
    cut_stack(stack, p, i0 - 1);
    list
}

/// `_List(length)`: `length` nulls, fixed length. The parameter has no type in
/// the declaration (`external factory _List(length)`): it arrives as a `Ref`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_List_allocate(length: i64, tuple: i64) -> i64 {
    // The type and its shape first: they only read (and intern into) the
    // universe, so the list needs no root while they are computed, and the list
    // is built in its shape and typed under one borrow of the heap. A fresh list
    // holds only nulls, which a compact shape keeps as zero bits — what
    // `adjust_list_shape` would have converted them to.
    let ty = tuple_list_type(tuple, Some(i64::from(crate::layout::cid::LIST)));
    let shape = ty.map_or(crate::lists::Element::General, type_list_shape_memo);
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let n = if crate::layout::smi::is_smi(length) {
            crate::layout::smi::value(length)
        } else {
            heap.int_of(length).unwrap_or(0)
        }
        .max(0) as usize;
        let h = heap.new_list(crate::layout::cid::LIST, n, shape);
        if let Some(ty) = ty {
            heap.set_metadata(h, ty + 1);
        }
        h
    })
}

/// `_List.length` / `_ImmutableList.length`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_List_getLength(this: i64) -> i64 {
    list_len(this)
}

/// `_List._setIndexed(i, v)`, with the index check.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_List_setIndexed(this: i64, ix: i64, value: i64) {
    write_index(this, ix, value);
}

/// The write of `_setIndexed` (`_List` and `_GrowableList`): `RangeError (index)`
/// outside the length; the value goes in unboxed into a compact list that holds
/// it, or decompacts it (`Heap::list_set`).
fn write_index(this: i64, ix: i64, value: i64) {
    if outside_list(ix, list_len(this), "index") {
        return;
    }
    HEAP.with(|heap| {
        heap.borrow_mut()
            .list_set(this, ix as usize, crate::heap::Value::Ref(value))
    });
}

/// `list[i]` of `_List`, `_ImmutableList` and `_GrowableList` (the VM's `[]`
/// intrinsic), with the index check. A compact element comes out in a box (`Smi`,
/// `_Mint`, `_Double`, `bool`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_list_get(this: i64, ix: i64) -> i64 {
    if outside_list(ix, list_len(this), "length") {
        return 0;
    }
    HEAP.with(|heap| heap.borrow_mut().list_get_ref(this, ix as usize))
}

/// The `how_many` elements of `of` from `start` into a new fixed list (`cid`) of
/// the same shape.
fn list_slice(of: i64, start: i64, how_many: i64, cid: i32) -> i64 {
    let (start, how_many) = (start.max(0) as usize, how_many.max(0) as usize);
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let shape = heap.list_shape(of);
        // `of` is an argument of the native: generated code keeps it rooted.
        let h = heap.new_list(cid, how_many, shape);
        heap.list_copy(of, start, h, 0, how_many);
        h
    })
}

/// `_List._sliceInternal(start, count, needsTypeArgument)`: a new `_List`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_List_slice(
    this: i64,
    start: i64,
    how_many: i64,
    _type: u8,
) -> i64 {
    let h = list_slice(this, start, how_many, crate::layout::cid::LIST);
    type_copied_list(h, this, crate::layout::cid::LIST);
    h
}

/// `_ImmutableList._from(from, offset, length)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_ImmutableList_from(
    of: i64,
    start: i64,
    how_many: i64,
    tuple: i64,
) -> i64 {
    let h = list_slice(of, start, how_many, crate::layout::cid::IMMUTABLE_LIST);
    type_tuple_list(h, tuple, crate::layout::cid::IMMUTABLE_LIST);
    h
}

/// `_GrowableList._withData(data)`: length 0 over the storage `data` (shared, as
/// in the VM: the following `_setLength` exposes its elements).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_GrowableList_allocate(data: i64, tuple: i64) -> i64 {
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // `data` is an argument of the native: generated code keeps it rooted.
        let h = heap.allocate_instance(crate::layout::cid::GROWABLE_LIST, 2);
        heap.list_set_data(h, data);
        h
    });
    type_tuple_list(h, tuple, crate::layout::cid::GROWABLE_LIST);
    h
}

/// `_GrowableList._capacity`: the storage's length.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_GrowableList_getCapacity(this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().list_capacity(this) as i64)
}

/// `_GrowableList.length`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_GrowableList_getLength(this: i64) -> i64 {
    list_len(this)
}

/// `_GrowableList._setLength(n)`: only the field (the positions between the old
/// length and `n` are already null — the storage is born zeroed and the shrinking
/// `length =` clears them). The SDK grows first (`_grow`); an `n` beyond the
/// capacity reserves, for robustness.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_GrowableList_setLength(this: i64, n: i64) {
    let n = n.max(0) as usize;
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if n > heap.list_capacity(this) {
            heap.list_reserve(this, n);
        }
        heap.list_set_len(this, n);
    });
}

/// `_GrowableList._setData(data)`: the storage becomes `data` (with a barrier).
/// The VM's `_shrink` builds `data` without a type (general); the compact shape the
/// list had comes back for the new storage, without allocating.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_GrowableList_setData(this: i64, data: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if clear_to_shared_empty(&mut heap, this, data) {
            return;
        }
        let shape = heap.list_shape(this);
        heap.list_set_data(this, data);
        // A compact list keeps its shape, also when shrunk to the shared
        // `_emptyList` (`clear()`): an empty storage takes the shared empty one of
        // the shape (`Heap::empty_storages`), with no allocation. Without it, a
        // `List<int>` buffer cleared once (the HTTP parser's, per request) stayed
        // general, and every later `add` took the boxed, dispatched path.
        if shape != crate::lists::Element::General {
            heap.list_adjust_shape(this, shape);
        }
    });
}

/// The `clear()` of a compact list (`_setData(_emptyList)` from `_shrink`), read and
/// written through the headers: the shared empty storage of the list's shape takes
/// the place of the new empty one directly, the result the general path below
/// reaches through `list_adjust_shape` with roots and checked writes (the HTTP
/// parser clears four `List<int>` buffers per request). Tracing memory without
/// handle validation, and only once that shared storage exists; false otherwise.
#[allow(unsafe_code)]
fn clear_to_shared_empty(heap: &mut Heap, this: i64, data: i64) -> bool {
    use crate::layout::{HANDLE_OFFSET, Header, cid, flags};
    if heap.arc.is_some()
        || heap.validate_handles
        || !crate::layout::is_object(this)
        || !crate::layout::is_object(data)
    {
        return false;
    }
    let b = (this - HANDLE_OFFSET) as *mut Header;
    // SAFETY: an object handle the generated code passed (the receiver of
    // `_setData`): a live block.
    let (class, f) = unsafe { ((*b).class_id, (*b).flags) };
    if class != cid::GROWABLE_LIST || f & flags::OUT_OF_LINE != 0 {
        return false;
    }
    // SAFETY: a `_GrowableList` has two inline fields; field 1 is its storage, a
    // live `_List`, and so is `data`, whose body word 0 is its capacity.
    let (field, old_flags, capacity) = unsafe {
        let field = crate::space::fields_of(b).add(1);
        let old = (*field - HANDLE_OFFSET) as *const Header;
        let new = (data - HANDLE_OFFSET) as *mut Header;
        (field, (*old).flags, *crate::space::fields_of(new))
    };
    let shape = crate::lists::Element::from_flags(old_flags);
    if shape == crate::lists::Element::General || capacity != 0 {
        return false;
    }
    let shared = heap.empty_storages[shape.code() as usize];
    if shared == 0 {
        return false;
    }
    // SAFETY: field 1 is already a reference in the list's map; the shared storage
    // is a root, and the list may be old (remembered below).
    unsafe { *field = shared };
    heap.objects_remember(b);
    true
}

/// The overlay's `_fillList(list, value)` (`array.dart`): the elements of
/// `_List.filled`/`_GrowableList.filled`, at once. `value` is already of the
/// list's `E` (the factory's parameter); it goes in unboxed into a compact list,
/// and the same object into every position of a general one (the VM's
/// `List.filled`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_List_fill(list: i64, value: i64) {
    HEAP.with(|heap| {
        heap.borrow_mut()
            .list_fill(list, crate::heap::Value::Ref(value))
    });
}

/// The overlay's `_copyElements(dest, origin, n)` (`array.dart`): the first `n`
/// elements of `origin` into the first `n` of `dest`, two core lists (`List.of`,
/// `toList` and the copies of `_List`/`_GrowableList`). Between lists of the same
/// shape, the words (with the block barrier on a general one); between different
/// shapes, element by element (`Heap::list_copy`). `dest` was just created with its
/// `E`, and `origin` is an `Iterable<E>`: every element fits in the destination; one
/// that did not fit would decompact it, as through `[]=`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_List_copy(dest: i64, origin: i64, n: i64) {
    let n = usize::try_from(n).unwrap_or(0);
    if n == 0 {
        return;
    }
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if !heap.is_list(origin) || !heap.is_list(dest) {
            return;
        }
        let n = n.min(heap.list_len(origin)).min(heap.list_len(dest));
        heap.list_copy(origin, 0, dest, 0, n);
    });
}

/// The overlay's `_GrowableList._grow(capacity)`
/// (`native_sdk/core/growable_array.dart`): a storage of `capacity`, of the same
/// shape, with the elements up to the length (the VM's `_grow`, which copied through
/// `[]=` and handed over to `_setData`, in a single copy).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_GrowableList_reserve(this: i64, capacity: i64) {
    let cap = usize::try_from(capacity).unwrap_or(0);
    HEAP.with(|heap| heap.borrow_mut().list_reserve(this, cap));
}

/// `_GrowableList._setIndexed(i, v)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_GrowableList_setIndexed(this: i64, ix: i64, value: i64) {
    write_index(this, ix, value);
}

/// `makeListFixedLength(list)`: a new `_List` with the elements of `list` (the
/// class does not change after publication, §2.16; the SDK's callers use the
/// returned value).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Internal_makeListFixedLength(list: i64) -> i64 {
    // `list` is an argument of the native: generated code keeps it rooted.
    let h = HEAP.with(|heap| heap.borrow_mut().fixed_list_of(list));
    type_copied_list(h, list, crate::layout::cid::LIST);
    h
}

/// `makeFixedListUnmodifiable(list)`: a new `_ImmutableList` with the elements of
/// `list` (the VM swaps the cid in place, `Array::MakeImmutable`; not here, because
/// `df.class` reads the cid with `!invariant.load`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Internal_makeFixedListUnmodifiable(list: i64) -> i64 {
    let h = HEAP.with(|heap| heap.borrow_mut().unmodifiable_list_of(list));
    type_copied_list(h, list, crate::layout::cid::IMMUTABLE_LIST);
    h
}

#[cfg(test)]
mod tests_runtime_type {
    use super::*;

    #[test]
    fn native_returns_canonical_value_type() {
        dartforge_rti_runtime_class(0, 101); // int
        dartforge_rti_runtime_class(3, 102); // String
        let number = dartforge_box_int(7);
        let text = allocate_str("sete");
        let number_type = dartforge_native_Object_runtimeType(number);
        assert_eq!(
            number_type,
            dartforge_native_Object_runtimeType(dartforge_box_int(9))
        );
        assert_ne!(number_type, dartforge_native_Object_runtimeType(text));
        assert_eq!(
            number_type,
            dartforge_rti_object_type(dartforge_rti_of_value(number))
        );
    }
}

/// A `_List` with the elements of the table `data` (`lower/literais.rs`): large
/// collection literals of constant scalars arrive as data, and not as one
/// instruction per element. Each element: `n` (null), `t`/`f` (bool), `i` + 8 bytes
/// (int, little-endian), `s` + 4 size bytes + the WTF-8 bytes (a string, canonical
/// like a literal's). The Dart support (`_dartforgeFillList`…) walks it and
/// discards it.
///
/// # Safety
/// `data` points to `len` readable bytes; `getters`, to a vector with the address
/// of each getter the table cites (`g` + 4 index bytes: a program constant,
/// `dfc.<hash>.get`, without arguments).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_list_from_table_g(
    data: *const u8,
    len: i64,
    getters: *const extern "C" fn() -> i64,
) -> i64 {
    // SAFETY: guaranteed by the caller (module constants).
    let b = unsafe { std::slice::from_raw_parts(data, usize::try_from(len).unwrap_or(0)) };
    // The elements, without the heap borrowed: the getter runs Dart code.
    enum Item<'a> {
        Value(crate::heap::Value),
        Text(&'a [u8]),
        Getter(usize),
    }
    let word = |b: &[u8], i: usize, n: usize| {
        let mut w = [0u8; 8];
        w[..n].copy_from_slice(&b[i..i + n]);
        u64::from_le_bytes(w)
    };
    let mut items: Vec<Item<'_>> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let tag = b[i];
        i += 1;
        items.push(match tag {
            b'n' => Item::Value(crate::heap::Value::Ref(0)),
            b't' => Item::Value(crate::heap::Value::Bool(true)),
            b'f' => Item::Value(crate::heap::Value::Bool(false)),
            b'i' => {
                i += 8;
                Item::Value(crate::heap::Value::Int(word(b, i - 8, 8) as i64))
            }
            b'd' => {
                i += 8;
                Item::Value(crate::heap::Value::Double(f64::from_bits(word(
                    b,
                    i - 8,
                    8,
                ))))
            }
            b's' => {
                let n = word(b, i, 4) as usize;
                i += 4 + n;
                Item::Text(&b[i - n..i])
            }
            b'g' => {
                i += 4;
                Item::Getter(word(b, i - 4, 4) as usize)
            }
            _ => panic!("compiler bug: collection table with marker {tag}"),
        });
    }
    let h = HEAP.with(|heap| {
        heap.borrow_mut().new_list(
            crate::layout::cid::LIST,
            items.len(),
            crate::lists::Element::General,
        )
    });
    // The list stays rooted while the strings and the getters allocate.
    with_roots(&[h], || {
        for (k, item) in items.into_iter().enumerate() {
            let v = match item {
                Item::Value(v) => v,
                // Literal strings are canonical and permanent (the `literals`
                // table is a root).
                Item::Text(t) => {
                    let t = Text::from_wtf8(t);
                    crate::heap::Value::Ref(
                        HEAP.with(|heap| heap.borrow_mut().string_literal(t.view())),
                    )
                }
                Item::Getter(g) => {
                    // SAFETY: the index is that of a vector entry (the compiler
                    // writes both together); the getter is a module function.
                    let f = unsafe { *getters.add(g) };
                    let v = dart_r0(f as usize);
                    if dartforge_exception_pending() != 0 {
                        return;
                    }
                    crate::heap::Value::Ref(v)
                }
            };
            // The boxes of `int`s outside the `Smi` range allocate: `list_set` roots `h`.
            HEAP.with(|heap| heap.borrow_mut().list_set(h, k, v));
        }
    });
    h
}

/// # Safety
/// `data` points to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_list_from_table(data: *const u8, len: i64) -> i64 {
    // SAFETY: guaranteed by the caller (a module constant).
    let b = unsafe { std::slice::from_raw_parts(data, usize::try_from(len).unwrap_or(0)) };
    let mut items: Vec<crate::heap::Value> = Vec::new();
    let mut i = 0;
    let word = |b: &[u8], i: usize, n: usize| {
        let mut w = [0u8; 8];
        w[..n].copy_from_slice(&b[i..i + n]);
        u64::from_le_bytes(w)
    };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        while i < b.len() {
            let tag = b[i];
            i += 1;
            let v = match tag {
                b'n' => crate::heap::Value::Ref(0),
                b't' => crate::heap::Value::Bool(true),
                b'f' => crate::heap::Value::Bool(false),
                b'i' => {
                    let x = word(b, i, 8) as i64;
                    i += 8;
                    crate::heap::Value::Int(x)
                }
                b'd' => {
                    let x = f64::from_bits(word(b, i, 8));
                    i += 8;
                    crate::heap::Value::Double(x)
                }
                b's' => {
                    let n = word(b, i, 4) as usize;
                    i += 4;
                    // Literal strings are canonical and permanent (the `literals`
                    // table is a root): no collection in the middle loses them.
                    let t = Text::from_wtf8(&b[i..i + n]);
                    let h = heap.string_literal(t.view());
                    i += n;
                    crate::heap::Value::Ref(h)
                }
                _ => panic!("compiler bug: collection table with marker {tag}"),
            };
            items.push(v);
        }
        let h = heap.new_list(
            crate::layout::cid::LIST,
            items.len(),
            crate::lists::Element::General,
        );
        // The boxes of `int`s outside the `Smi` range allocate: `list_set` roots `h`.
        for (k, v) in items.into_iter().enumerate() {
            heap.list_set(h, k, v);
        }
        h
    })
}

// ─── Unified space (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ────────────────────

/// The list literal (`AllocList`, `llvm/listas_ir.rs`): `n` words in `words`, of
/// class `cid` (`_List`, `_ImmutableList` or `_GrowableList` of capacity `n`).
/// `shape`:
///
/// * `0`–`3` (`Element::code`): all words already in the shape's representation
///   (`Ref` in the general one; `i64`, `f64` bits or 0/1 in the compact one);
/// * any other value: the address of `n` bytes with the representation of each word
///   (the tag of the pair ABI: 1 `int`, 2 `bool`, 3 `Ref`, 4 `double` bits), for a
///   mixed literal; the list comes out general, with the boxes made here (generated
///   code does not box, and a box made before the list would be unrooted in the
///   following allocation).
///
/// The words' `Ref`s are operands of the instruction, rooted by generated code
/// during the call. Does not throw.
///
/// # Safety
/// `words` points to `n` readable words; a `shape` outside `0..=3` points to `n`
/// readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_list_new(
    words: *const i64,
    n: i64,
    cid: i64,
    shape: i64,
) -> i64 {
    // SAFETY: this entry's contract is the same as the common implementation's.
    unsafe { list_literal(words, n, cid, shape, false) }
}

/// Creates a list literal and hands over an independent code owner. The Ref
/// elements are copied, without consuming the caller's owners, and stay rooted
/// during all allocations. Neither runs Dart nor leaves a pending exception.
///
/// # Safety
/// `n` must be non-negative and `words` contain `n` readable words. Outside
/// `0..=3`, `shape` points to `n` readable tags (1 int, 2 bool, 3 Ref, 4 double).
/// References must be alive; bool uses only 0/1.
///
/// # Panics
/// Invalid length, class, tag or bool; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_arc_list_new_owned_v1([42].as_ptr(), 1, 10, 1) };
/// assert_eq!(dartforge_arc_list_len_v1(list), 1);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_list_new_owned_v1(
    words: *const i64,
    n: i64,
    cid: i64,
    shape: i64,
) -> i64 {
    assert!(n >= 0, "negative literal length");
    assert!(matches!(cid, 8..=10), "invalid literal class");
    // SAFETY: buffers and references valid by the entry's contract.
    unsafe { list_literal(words, n, cid, shape, true) }
}

unsafe fn list_literal(words: *const i64, n: i64, cid: i64, shape: i64, owned: bool) -> i64 {
    use crate::lists::Element;
    let n = usize::try_from(n).unwrap_or(0);
    // SAFETY: guaranteed by the caller (generated code's `alloca`).
    let p: &[i64] = if n == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(words, n) }
    };
    let cid = i32::try_from(cid).expect("compiler bug: invalid list cid");
    let (e, tags): (Element, Option<&[u8]>) = match Element::from_code(shape) {
        Some(e) => (e, None),
        // SAFETY: guaranteed by the caller (the literal's tag vector).
        None => (
            Element::General,
            Some(unsafe { std::slice::from_raw_parts(shape as usize as *const u8, n) }),
        ),
    };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // Homogeneous tracing keeps the direct fill, without an extra vector.
        let refs: Vec<i64> = if !owned && tags.is_none() {
            Vec::new()
        } else {
            p.iter()
                .enumerate()
                .map(|(k, &w)| {
                    let tag = tags.map_or(if e == Element::General { 3 } else { 0 }, |t| t[k]);
                    if owned {
                        assert!(
                            tags.is_none() || matches!(tag, 1..=4),
                            "invalid literal tag"
                        );
                        assert!(
                            !(tag == 2 || e == Element::Bool) || matches!(w, 0 | 1),
                            "invalid literal bool"
                        );
                    }
                    if tag == 3 { w } else { 0 }
                })
                .collect()
        };
        let frame = owned.then(|| {
            let frame = heap.push_owning_frame(refs.len());
            for (k, &r) in refs.iter().enumerate() {
                heap.set_root(frame, k, r);
            }
            frame
        });
        let h = if cid == crate::layout::cid::GROWABLE_LIST {
            heap.new_growable(n, n, e)
        } else {
            heap.new_list(cid, n, e)
        };
        let a = heap.list_data(h);
        match tags {
            None => {
                // Freshly allocated, young: the words go in without a barrier.
                if owned && e == Element::General {
                    heap.write_refs(a, 1, p);
                } else {
                    heap.words_mut(a)[1..1 + n].copy_from_slice(p);
                }
            }
            Some(tags) => {
                // The `Ref`s first (without allocation), then the boxes (which may
                // collect; `list_set` roots `h`).
                if owned {
                    heap.write_refs(a, 1, &refs);
                } else {
                    heap.words_mut(a)[1..1 + n].copy_from_slice(&refs);
                }
                for (k, (&w, &t)) in p.iter().zip(tags).enumerate() {
                    let v = match t {
                        1 => crate::heap::Value::Int(w),
                        2 => crate::heap::Value::Bool(w != 0),
                        4 => crate::heap::Value::Double(f64::from_bits(w as u64)),
                        _ => continue,
                    };
                    heap.list_set(h, k, v);
                }
            }
        }
        if let Some(frame) = frame {
            heap.retain_code_owner(h);
            heap.pop_frame(frame);
        }
        h
    })
}

/// The `add` of a `_GrowableList` with the `Ref` `v` (generated code's path when
/// the storage is full or does not hold `v` unboxed; `lower/intrinsecos.rs`,
/// `lower/literais.rs`): grows to `(capacity * 2) | 3` if needed and writes
/// (decompacting, if that is the case). Returns `list`. Does not throw; may collect
/// (`list` and `v` are operands, rooted by generated code).
/// The return is borrowed from the first argument; it does not create an
/// independent owner. The value is stored without consuming the caller's owner; in
/// the general shape, it copies the Ref edge. Growth and decompaction invalidate
/// previous borrows of the storage.
///
/// # Panics
/// A non-growable receiver, dead references or an invalid layout; aborts at the C
/// boundary. The caller must keep both arguments alive.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_list_new(std::ptr::null(), 0, 10, 0) };
/// dartforge_arc_retain(list);
/// let value = dartforge_arc_box_int_owned_v1(i64::MAX);
/// assert_eq!(dartforge_list_append(list, value), list);
/// dartforge_arc_release(value);
/// assert_eq!(dartforge_arc_list_len_v1(list), 1);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_append(list: i64, v: i64) -> i64 {
    HEAP.with(|heap| {
        heap.borrow_mut()
            .list_push(list, crate::heap::Value::Ref(v))
    });
    list
}

#[cfg(test)]
mod literal_owned_tests {
    use super::*;

    #[test]
    fn json_close_list_copies_nested_ranges_and_keeps_references_alive() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            let previous_mode = ARC_MEMORY.with(|mode| mode.replace(false));
            // Raw native writes use the runtime's TLS mode as well as Heap ARC.
            if arc {
                dartforge_memory_arc_v1();
            }
            for count in [0, 1, 257] {
                let stack = HEAP.with(|h| {
                    h.borrow_mut()
                        .new_growable(0, count + 3, crate::lists::Element::General)
                });
                dartforge_arc_retain(stack);
                let model = HEAP.with(|h| {
                    h.borrow_mut()
                        .new_growable(0, 0, crate::lists::Element::General)
                });
                dartforge_arc_retain(model);
                let child = HEAP.with(|h| {
                    h.borrow_mut()
                        .allocate_str("JSON child shared across array entries")
                });
                dartforge_arc_retain(child);
                let prefix = crate::layout::smi::of(17).unwrap();
                let mark = crate::layout::smi::of(37).unwrap();
                assert_eq!(dartforge_native_DartForge_json_push(stack, prefix), 1);
                assert_eq!(dartforge_native_DartForge_json_push(stack, mark), 2);
                let expected: Vec<i64> = (0..count)
                    .map(|i| match i % 3 {
                        0 => child,
                        1 => 0,
                        _ => crate::layout::smi::of(i as i64).unwrap(),
                    })
                    .collect();
                for &value in &expected {
                    dartforge_native_DartForge_json_push(stack, value);
                }
                let list = dartforge_native_DartForge_json_close_list(stack, 2, model);
                assert_ne!(list, 0);
                dartforge_arc_retain(list);
                dartforge_arc_release(child);
                dartforge_arc_collect();
                HEAP.with(|h| {
                    let heap = h.borrow();
                    assert_eq!(heap.list_len(stack), 1);
                    let storage = heap.list_data(stack);
                    assert_eq!(heap.words(storage)[1], prefix);
                    assert!(
                        heap.words(storage)[2..2 + count + 1]
                            .iter()
                            .all(|&v| v == 0)
                    );
                    assert_eq!(heap.list_len(list), count);
                    assert_eq!(heap.list_capacity(list), count);
                    assert_eq!(heap.metadata(list), heap.metadata(model));
                    let storage = heap.list_data(list);
                    assert_eq!(&heap.words(storage)[1..1 + count], expected.as_slice());
                    if count != 0 {
                        assert!(heap.is_live_object(child));
                    }
                });
                for value in [list, stack, model] {
                    dartforge_arc_release(value);
                }
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observe_heap_v1(list), 0);
                assert_eq!(dartforge_arc_observe_heap_v1(child), 0);
            }
            HEAP.with(|h| {
                h.replace(previous);
            });
            ARC_MEMORY.with(|mode| mode.set(previous_mode));
        }
    }

    #[test]
    fn mixed_literal_roots_refs_counts_edges_and_preserves_boxes() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc {
                HEAP.with(|h| h.borrow_mut().enable_arc());
            }
            for cid in [8, 9, 10] {
                let child =
                    HEAP.with(|h| h.borrow_mut().allocate_str("element without a code owner"));
                let bits = 0xfff8_1234_5678_9abc_u64;
                let words = [child, i64::MAX, bits as i64, 1, child, 0];
                let tags = [3u8, 1, 4, 2, 3, 3];
                let list = unsafe {
                    dartforge_arc_list_new_owned_v1(words.as_ptr(), 6, cid, tags.as_ptr() as i64)
                };
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observe_heap_v1(list), 3);
                assert_eq!(dartforge_arc_observe_heap_v1(child), 1);
                HEAP.with(|h| {
                    let heap = h.borrow();
                    assert_eq!(heap.list_len(list), 6);
                    let a = heap.list_data(list);
                    let w = heap.words(a);
                    assert_eq!(w[1], child);
                    assert_eq!(w[5], child);
                    assert_eq!(w[6], 0);
                    assert_eq!(heap.value(w[2]), crate::heap::Value::Int(i64::MAX));
                    let crate::heap::Value::Double(d) = heap.value(w[3]) else {
                        panic!("double box expected")
                    };
                    assert_eq!(d.to_bits(), bits);
                    assert_eq!(heap.value(w[4]), crate::heap::Value::Bool(true));
                });
                dartforge_arc_release(list);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observe_heap_v1(list), 0);
                assert_eq!(dartforge_arc_observe_heap_v1(child), 0);
            }
            HEAP.with(|h| {
                h.replace(previous);
            });
        }
    }

    #[test]
    fn empty_and_compact_literal_preserves_bits_until_owner_release() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc {
                HEAP.with(|h| h.borrow_mut().enable_arc());
            }
            for cid in [8, 9, 10] {
                for (shape, words) in [
                    (0, vec![]),
                    (1, vec![i64::MIN, i64::MAX]),
                    (2, vec![i64::MIN, 0x7ff8_1234_5678_9abc]),
                    (3, vec![0, 1]),
                ] {
                    let list = unsafe {
                        dartforge_arc_list_new_owned_v1(
                            words.as_ptr(),
                            words.len() as i64,
                            cid,
                            shape,
                        )
                    };
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observe_heap_v1(list), 3);
                    HEAP.with(|h| {
                        let heap = h.borrow();
                        let data = heap.list_data(list);
                        assert_eq!(&heap.words(data)[1..1 + words.len()], &words);
                    });
                    dartforge_arc_release(list);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observe_heap_v1(list), 0);
                }
            }
            HEAP.with(|h| {
                h.replace(previous);
            });
        }
    }
}
