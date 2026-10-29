// ABI of tokens and owning slots used by the generated code.
// The observational stack does not replace these owners. Ownership operations
// neither collect nor run Dart; collection has a separate explicit safepoint.
// The owned box factory is an allocation operation, with its own safepoint.
// The Ref throw publishes a runtime root and may allocate the stack trace.

fn audited_list_map(heap: &Heap, list: i64) -> Result<(i64, usize, usize), &'static str> {
    use crate::layout::cid;
    if !heap.is_list(list) { return Err("receiver is not a live list"); }
    let (data, length) = if heap.header(list).class_id == cid::GROWABLE_LIST {
        let object = heap.object(list).ok_or("growable list without instance map")?;
        if object.len() != 2 { return Err("invalid growable list layout"); }
        let (n, mark_n) = object.get(0).ok_or("missing length")?;
        let (data, mark_data) = object.get(1).ok_or("missing storage")?;
        if mark_n || !mark_data { return Err("invalid growable list map"); }
        (data, usize::try_from(n).map_err(|_| "negative length")?)
    } else {
        let n = *heap.words(list).first().ok_or("missing length")?;
        (list, usize::try_from(n).map_err(|_| "negative length")?)
    };
    if !heap.is_list(data) || !cid::is_fixed_list(heap.header(data).class_id) {
        return Err("storage is not a live fixed list");
    }
    let words = heap.words(data);
    let capacity = usize::try_from(*words.first().ok_or("missing capacity")?)
        .map_err(|_| "negative capacity")?;
    if capacity > words.len().saturating_sub(1) {
        return Err("capacity outside the storage");
    }
    Ok((data, length, capacity))
}

fn audited_list_storage(heap: &Heap, list: i64) -> Result<(i64, usize), &'static str> {
    let (data, length, capacity) = audited_list_map(heap, list)?;
    if length > capacity { return Err("length outside the storage"); }
    Ok((data, length))
}

fn read_audited_ref_element(heap: &Heap, list: i64, ix: i64) -> Result<i64, &'static str> {
    use crate::layout::flags;
    let (data, length) = audited_list_storage(heap, list)?;
    if heap.header(data).flags & flags::FORMAT != flags::REFS {
        return Err("storage does not hold references");
    }
    let words = heap.words(data);
    let ix = usize::try_from(ix).map_err(|_| "negative index")?;
    if ix >= length { return Err("index outside the list"); }
    Ok(words[1 + ix])
}

/// Reads the scalar length of a live list without changing its ownership.
///
/// Accepts fixed, immutable and growable lists, general or compact. Checks
/// the storage map and bounds without allocating, collecting or running Dart.
/// The received reference stays borrowed during the call.
///
/// # Panics
/// Dead receiver, invalid class, map or length; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // SAFETY: Ref elements valid during the list creation.
/// let list = unsafe { dartforge_list_new([0, 0].as_ptr(), 2, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_list_len_v1(list), 2);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_list_len_v1(list: i64) -> i64 {
    HEAP.with(|h| {
        let heap = h.borrow();
        let (_, length) = audited_list_storage(&heap, list)
            .expect("invalid ARC list length");
        i64::try_from(length).expect("length outside i64")
    })
}

/// Borrows the Ref storage, kept alive by the owner of the received list.
///
/// Checks class, map and bounds; does not allocate, collect, retain or run Dart.
/// The borrow loses validity after an operation that may swap edges.
///
/// # Panics
/// Dead list or invalid layout; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // SAFETY: one Ref element valid during the creation.
/// let list = unsafe { dartforge_list_new([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_list_storage_ref_v1(list), list);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_list_storage_ref_v1(list: i64) -> i64 {
    HEAP.with(|h| audited_list_storage(&h.borrow(), list)
        .expect("invalid ARC storage").0)
}

/// Reads the scalar shape of a live list's storage.
///
/// Does not allocate, collect, retain or run Dart; the reference stays borrowed.
///
/// # Panics
/// Dead list or invalid layout; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_list_new([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_list_shape_v1(list), 4);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_list_shape_v1(list: i64) -> i64 {
    HEAP.with(|h| {
        let heap = h.borrow();
        let (data, _) = audited_list_storage(&heap, list).expect("invalid ARC shape");
        i64::from(heap.header(data).flags & (crate::layout::flags::FORMAT | crate::layout::flags::ELEMENT))
    })
}

/// Reads the scalar capacity of a live list's storage.
///
/// Does not allocate, collect, retain or run Dart; the reference stays borrowed.
///
/// # Panics
/// Dead list or invalid layout; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_list_new([0, 0].as_ptr(), 2, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_list_capacity_v1(list), 2);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_list_capacity_v1(list: i64) -> i64 {
    HEAP.with(|h| {
        let heap = h.borrow();
        let (data, _) = audited_list_storage(&heap, list).expect("invalid ARC capacity");
        heap.list_len(data) as i64
    })
}

fn check_arc_list_len(heap: &Heap, list: i64, new: i64) -> Result<usize, &'static str> {
    let (_, _, capacity) = audited_list_map(heap, list)?;
    if heap.header(list).class_id != crate::layout::cid::GROWABLE_LIST {
        return Err("mutable length requires a growable list");
    }
    let new = usize::try_from(new).map_err(|_| "negative length")?;
    if new > capacity { return Err("length outside the storage"); }
    Ok(new)
}

/// Writes the scalar length of a growable list within the capacity.
///
/// Preserves the SDK's `_setData` followed by `_setLength` sequence, even
/// when the old length temporarily exceeds the new capacity.
/// Does not allocate, collect or run Dart; invalidates earlier borrows.
///
/// # Panics
/// Invalid class, map or new length; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_list_new([0].as_ptr(), 1, 10, 0) };
/// dartforge_arc_retain(list);
/// dartforge_arc_list_write_len_v1(list, 0);
/// assert_eq!(dartforge_arc_list_len_v1(list), 0);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_list_write_len_v1(list: i64, new: i64) {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let new = check_arc_list_len(&heap, list, new).expect("invalid ARC length write");
        heap.list_set_len(list, new);
        heap.arc_settle();
    })
}

fn check_arc_list_bits(heap: &Heap, data: i64, ix: i64, bits: i64, shape: i64)
    -> Result<usize, &'static str> {
    use crate::layout::{cid, flags};
    let (storage, length) = audited_list_storage(heap, data)?;
    if storage != data || heap.header(data).class_id != cid::LIST {
        return Err("write requires a mutable fixed storage");
    }
    let f = heap.header(data).flags & (flags::FORMAT | flags::ELEMENT);
    if shape != i64::from(f) || ![flags::RAW | flags::ELEMENT_INT,
        flags::RAW | flags::ELEMENT_DOUBLE, flags::RAW | flags::ELEMENT_BOOL].contains(&f) {
        return Err("incompatible scalar shape");
    }
    if f == flags::RAW | flags::ELEMENT_BOOL && ![0, 1].contains(&bits) {
        return Err("non-canonical boolean");
    }
    let i = usize::try_from(ix).map_err(|_| "negative index")?;
    if i >= length { return Err("index outside the storage"); }
    Ok(i)
}

/// Writes int, double or bool bits into a mutable compact storage.
///
/// Checks shape and index; `shape` uses the FORMA/ELEMENTO bits of the layout.
/// Does not box, allocate, collect or run Dart; invalidates earlier borrows.
///
/// # Panics
/// Invalid list, shape, index or boolean; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_list_new([0].as_ptr(), 1, 8, 1) };
/// dartforge_arc_retain(list);
/// dartforge_arc_list_write_bits_v1(list, 0, 42, 0x22);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_list_write_bits_v1(data: i64, ix: i64, bits: i64, shape: i64) {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let i = check_arc_list_bits(&heap, data, ix, bits, shape).expect("invalid ARC scalar write");
        heap.words_mut(data)[1 + i] = bits;
    })
}

/// Copies a borrowed reference into a general element, with the ARC/GC barrier.
///
/// Checks mutable storage, Ref shape and index. Does not allocate, collect or
/// run Dart; does not consume the value's owner and invalidates earlier borrows.
///
/// # Panics
/// Invalid list, index or dead reference; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_list_new([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(list);
/// dartforge_arc_list_write_ref_v1(list, 0, 0);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_list_write_ref_v1(data: i64, ix: i64, value: i64) {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        assert!(heap.is_list(data) && heap.header(data).class_id == crate::layout::cid::LIST,
            "ARC Ref write requires mutable storage");
        read_audited_ref_element(&heap, data, ix).expect("invalid ARC Ref write");
        assert!(!crate::layout::is_object(value) || heap.is_live_object(value), "dead ARC reference");
        heap.write_refs(data, 1 + ix as usize, &[value]);
        heap.arc_settle();
    })
}

/// Swaps the storage of a growable list if the new one has the same shape.
///
/// Returns 1 after copying the Ref edge with the barrier, or 0 without changing the list
/// when the shapes differ. Does not allocate, collect or run Dart. The swap may
/// precede `_setLength` when shrinking and invalidates earlier borrows.
///
/// # Panics
/// Invalid receiver, map or storage; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let list = unsafe { dartforge_list_new([0].as_ptr(), 1, 10, 0) };
/// dartforge_arc_retain(list);
/// let data = dartforge_arc_list_storage_ref_v1(list);
/// assert_eq!(dartforge_arc_list_write_data_v1(list, data), 1);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_list_write_data_v1(list: i64, data: i64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let (old_gen, _, _) = audited_list_map(&heap, list).expect("invalid ARC list map");
        assert_eq!(heap.header(list).class_id, crate::layout::cid::GROWABLE_LIST);
        let (fixed, _) = audited_list_storage(&heap, data).expect("invalid new ARC storage");
        assert_eq!(fixed, data, "ARC storage must be fixed");
        let mask = crate::layout::flags::FORMAT | crate::layout::flags::ELEMENT;
        if heap.header(old_gen).flags & mask != heap.header(data).flags & mask { return 0; }
        heap.list_set_data(list, data);
        heap.arc_settle();
        1
    })
}

/// Reads the bits of an int, double or bool element of a live compact list.
///
/// Checks fixed/immutable/growable list, map, compact shape and logical
/// bounds; bool comes out as 0 or 1. Does not allocate, collect, retain or run Dart; the
/// reference stays borrowed. The Dart index guards must already have been
/// executed.
///
/// # Panics
/// Invalid list/layout/shape/index; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // SAFETY: scalar elements during the creation of the compact list.
/// let list = unsafe { dartforge_list_new([7].as_ptr(), 1, 8, 1) };
/// dartforge_arc_retain(list);
/// dartforge_arc_list_write_bits_v1(list, 0, 42, 0x22);
/// assert_eq!(dartforge_arc_list_read_bits_v1(list, 0), 42);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_list_read_bits_v1(list: i64, ix: i64) -> i64 {
    HEAP.with(|h| {
        let heap = h.borrow();
        read_arc_list_bits(&heap, list, ix).expect("invalid ARC scalar read")
    })
}

fn read_arc_list_bits(heap: &Heap, list: i64, ix: i64) -> Result<i64, &'static str> {
    use crate::layout::flags;
    let (data, length) = audited_list_storage(heap, list)?;
    let f = heap.header(data).flags & (flags::FORMAT | flags::ELEMENT);
    if ![flags::RAW | flags::ELEMENT_INT, flags::RAW | flags::ELEMENT_DOUBLE,
        flags::RAW | flags::ELEMENT_BOOL].contains(&f) {
        return Err("storage is not compact");
    }
    let i = usize::try_from(ix).map_err(|_| "negative index")?;
    if i >= length { return Err("index outside the list"); }
    let bits = heap.words(data)[1 + i];
    if f == flags::RAW | flags::ELEMENT_BOOL && ![0, 1].contains(&bits) {
        return Err("non-canonical boolean");
    }
    Ok(bits)
}

/// Reads an element of the general list and returns an independent owner.
///
/// Checks a live fixed/immutable/growable list, map, Ref representation and
/// physical and logical bounds. Retains without allocating, collecting or running Dart.
/// The caller must consume the token; the Dart index guards must already have
/// been executed. Does not convert compact elements into references.
///
/// # Panics
/// Invalid list/layout/index or retention failure; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // SAFETY: one Ref element alive and aligned during the copy.
/// let list = unsafe { dartforge_list_new([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_list_ref_owned_v1(list, 0), 0);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_list_ref_owned_v1(list: i64, ix: i64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let value = read_audited_ref_element(&heap, list, ix).expect("invalid ARC list read");
        heap.retain_code_owner(value);
        value
    })
}

/// Reads an element of the general list borrowed from the list, without a code token.
///
/// The same checks as [`dartforge_arc_list_ref_owned_v1`]. It is the
/// ARC read without preparation (the legacy regime), in which nobody consumes
/// tokens: the element stays alive through the list and the frame roots.
///
/// # Panics
/// Invalid list/layout/index; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // SAFETY: one Ref element alive and aligned during the copy.
/// let list = unsafe { dartforge_list_new([0].as_ptr(), 1, 8, 0) };
/// dartforge_arc_retain(list);
/// assert_eq!(dartforge_arc_list_ref_v1(list, 0), 0);
/// dartforge_arc_release(list);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_list_ref_v1(list: i64, ix: i64) -> i64 {
    HEAP.with(|h| read_audited_ref_element(&h.borrow(), list, ix).expect("invalid ARC list read"))
}

#[cfg(test)]
mod tests_owned_ref_list {
    use super::*;
    use crate::{layout::{cid, smi}, lists::Element, heap::Value};

    #[test]
    fn append_borrows_receiver_and_preserves_roots_across_growth_and_decompaction() {
        for arc in [false, true] {
            for full_list in [false, true] {
                for shape in [Element::General, Element::Int, Element::Double, Element::Bool] {
                    for compatible in [false, true] {
                        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
                        let (list, value) = HEAP.with(|h| {
                            let mut heap = h.borrow_mut();
                            if arc { heap.enable_arc(); }
                            let list = heap.new_growable(usize::from(full_list), if full_list { 1 } else { 2 }, shape);
                            heap.retain_code_owner(list);
                            if full_list {
                                let data = heap.list_data(list);
                                heap.words_mut(data)[1] = match shape {
                                    Element::General => 0, Element::Int => i64::MIN,
                                    Element::Double => (-0.0_f64).to_bits() as i64, Element::Bool => 1,
                                };
                            }
                            let v = if !compatible || shape == Element::General {
                                Value::Ref(heap.allocate_instance(123, 1))
                            } else { match shape {
                                Element::Int => Value::Int(i64::MAX),
                                Element::Double => Value::Double(f64::from_bits(0xfff8_1234_5678_9abc)),
                                Element::Bool => Value::Bool(true), Element::General => unreachable!(),
                            }};
                            let value = heap.to_ref(v);
                            heap.retain_code_owner(value);
                            (list, value)
                        });
                        let stats = HEAP.with(|h| h.borrow().stats());
                        let r = dartforge_list_append(list, value);
                        assert_eq!(r, list);
                        HEAP.with(|h| {
                            let heap = h.borrow();
                            assert_eq!(heap.list_len(list), 1 + usize::from(full_list));
                            assert_eq!(heap.list_capacity(list), if full_list { 3 } else { 2 });
                            let general = !compatible || shape == Element::General;
                            assert_eq!(heap.list_shape(list), if general { Element::General } else { shape });
                            let last_one = heap.list_get(list, usize::from(full_list));
                            match shape {
                                _ if general => assert_eq!(last_one, Value::Ref(value)),
                                Element::Int => assert_eq!(last_one, Value::Int(i64::MAX)),
                                Element::Double => {
                                    let Value::Double(d) = last_one else { panic!("double element expected") };
                                    assert_eq!(d.to_bits(), 0xfff8_1234_5678_9abc);
                                },
                                Element::Bool => assert_eq!(last_one, Value::Bool(true)),
                                Element::General => unreachable!(),
                            }
                            if full_list || (!compatible && shape != Element::General) {
                                assert!(heap.stats().allocations > stats.allocations);
                                assert!(heap.stats().collections > stats.collections);
                            } else {
                                assert_eq!(heap.stats().allocations, stats.allocations);
                                assert_eq!(heap.stats().collections, stats.collections);
                            }
                        });
                        dartforge_arc_release(value);
                        dartforge_arc_collect();
                        if !compatible || shape == Element::General {
                            assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
                        }
                        // The borrowed return cannot leave an additional owner of the list.
                        dartforge_arc_release(list);
                        dartforge_arc_collect();
                        assert!(!HEAP.with(|h| h.borrow().is_live_object(r)));
                        if !compatible || shape == Element::General {
                            assert!(!HEAP.with(|h| h.borrow().is_live_object(value)));
                        }
                        HEAP.with(|h| h.replace(previous));
                    }
                }
            }
        }
    }

    #[test]
    fn compact_storage_preserves_bits_and_shrink_sequence() {
        for arc in [false, true] {
            for (shape, bits) in [(Element::Int, i64::MIN),
                (Element::Double, 0xfff8_1234_5678_9abc_u64 as i64), (Element::Bool, 1)] {
                let previous = HEAP.with(|h| h.replace(Heap::new(true)));
                let list = HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    if arc { heap.enable_arc(); }
                    let list = heap.new_growable(0, 3, shape);
                    heap.retain_code_owner(list);
                    list
                });
                let stats = HEAP.with(|h| h.borrow().stats());
                let data = dartforge_arc_list_storage_ref_v1(list);
                assert_ne!(data, list);
                assert_eq!(dartforge_arc_list_capacity_v1(list), 3);
                assert_eq!(dartforge_arc_list_len_v1(data), 3);
                assert_eq!(dartforge_arc_list_shape_v1(data), i64::from(shape.flags()));
                dartforge_arc_list_write_bits_v1(data, 0, bits, i64::from(shape.flags()));
                dartforge_arc_list_write_len_v1(list, 1);
                HEAP.with(|h| {
                    let heap = h.borrow();
                    assert_eq!(heap.words(data)[1], bits);
                    assert_eq!(heap.stats().allocations, stats.allocations);
                    assert_eq!(heap.stats().collections, stats.collections);
                });
                // Explicit copy before removing the edge that sustains the borrow.
                dartforge_arc_retain(data);
                let new = HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    let new = heap.new_list(cid::LIST, 0, shape);
                    heap.retain_code_owner(new);
                    new
                });
                assert_eq!(dartforge_arc_list_write_data_v1(list, new), 1);
                assert!(HEAP.with(|h| audited_list_storage(&h.borrow(), list).is_err()));
                dartforge_arc_list_write_len_v1(list, 0);
                assert_eq!(dartforge_arc_list_len_v1(list), 0);
                dartforge_arc_release(new);
                dartforge_arc_collect();
                assert!(HEAP.with(|h| h.borrow().is_live_object(data)));
                dartforge_arc_release(data);
                dartforge_arc_collect();
                assert!(!HEAP.with(|h| h.borrow().is_live_object(data)));
                assert!(HEAP.with(|h| h.borrow().is_live_object(new)));
                dartforge_arc_release(list);
                dartforge_arc_collect();
                assert!(!HEAP.with(|h| h.borrow().is_live_object(new)));
                HEAP.with(|h| h.replace(previous));
            }
        }
    }

    #[test]
    fn ref_storage_copies_edges_and_preserves_element_owner() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            let list = HEAP.with(|h| {
                let mut heap = h.borrow_mut();
                if arc { heap.enable_arc(); }
                let list = heap.new_growable(1, 3, Element::General);
                heap.retain_code_owner(list);
                list
            });
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            let data = dartforge_arc_list_storage_ref_v1(list);
            let stats = HEAP.with(|h| h.borrow().stats());
            dartforge_arc_list_write_ref_v1(data, 0, child);
            assert_eq!(HEAP.with(|h| h.borrow().stats().allocations), stats.allocations);
            assert_eq!(HEAP.with(|h| h.borrow().stats().collections), stats.collections);
            dartforge_arc_release(child);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(child)));
            let received = dartforge_arc_list_ref_owned_v1(data, 0);
            dartforge_arc_list_write_ref_v1(data, 0, 0);
            dartforge_arc_release(list);
            dartforge_arc_collect();
            assert_eq!(dartforge_unbox_int(received), i64::MAX);
            dartforge_arc_release(received);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(child)));
            HEAP.with(|h| h.replace(previous));
        }
    }

    #[test]
    fn writes_reject_invalid_shape_index_and_len_and_keep_shape() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let (list, new) = HEAP.with(|h| {
            let mut heap = h.borrow_mut();
            heap.enable_arc();
            let list = heap.new_growable(1, 2, Element::Bool);
            heap.retain_code_owner(list);
            let new = heap.new_list(cid::LIST, 1, Element::Int);
            heap.retain_code_owner(new);
            (list, new)
        });
        let data = dartforge_arc_list_storage_ref_v1(list);
        assert_eq!(dartforge_arc_list_write_data_v1(list, new), 0);
        assert_eq!(dartforge_arc_list_storage_ref_v1(list), data);
        HEAP.with(|h| {
            let heap = h.borrow();
            for (i, bits, shape) in [(-1, 1, 0x62), (2, 1, 0x62), (0, 2, 0x62),
                (0, 1, 0x22), (0, 1, 0x04)] {
                assert!(check_arc_list_bits(&heap, data, i, bits, shape).is_err());
            }
            assert!(check_arc_list_bits(&heap, list, 0, 1, 0x62).is_err());
            assert!(check_arc_list_len(&heap, data, 0).is_err());
            for n in [-1, 3] { assert!(check_arc_list_len(&heap, list, n).is_err()); }
            assert_eq!(heap.words(data)[1], 0);
        });
        dartforge_arc_release(new);
        dartforge_arc_release(list);
        dartforge_arc_collect();
        HEAP.with(|h| h.replace(previous));
    }

    #[test]
    fn general_and_compact_length_does_not_allocate_collect_or_retain() {
        for arc in [false, true] {
            for class in [cid::LIST, cid::IMMUTABLE_LIST, cid::GROWABLE_LIST] {
                for shape in [Element::General, Element::Int, Element::Double, Element::Bool] {
                    let previous = HEAP.with(|h| h.replace(Heap::new(true)));
                    let list = HEAP.with(|h| {
                        let mut heap = h.borrow_mut();
                        if arc { heap.enable_arc(); }
                        let list = if class == cid::GROWABLE_LIST {
                            heap.new_growable(2, 5, shape)
                        } else { heap.new_list(class, 2, shape) };
                        heap.retain_code_owner(list);
                        list
                    });
                    let stats = HEAP.with(|h| h.borrow().stats().clone());
                    assert_eq!(dartforge_arc_list_len_v1(list), 2);
                    HEAP.with(|h| {
                        let heap = h.borrow();
                        assert_eq!(heap.stats().allocations, stats.allocations);
                        assert_eq!(heap.stats().collections, stats.collections);
                    });
                    dartforge_arc_release(list);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observe_heap_v1(list), 0);
                    assert!(HEAP.with(|h| audited_list_storage(&h.borrow(), list).is_err()));
                    HEAP.with(|h| h.replace(previous));
                }
            }
        }
    }

    #[test]
    fn length_rejects_invalid_map_class_and_bounds() {
        let mut heap = Heap::new(true);
        let other = heap.allocate_instance(123, 2);
        heap.retain_code_owner(other);
        for receiver in [0, smi::of(3).unwrap(), i64::MAX, other] {
            assert!(audited_list_storage(&heap, receiver).is_err());
        }
        let fixed = heap.new_list(cid::LIST, 2, Element::Int);
        heap.retain_code_owner(fixed);
        for n in [-1, 3, i64::MAX] {
            heap.words_mut(fixed)[0] = n;
            assert!(audited_list_storage(&heap, fixed).is_err());
        }
        heap.words_mut(fixed)[0] = 2;
        let list = heap.new_growable(0, 3, Element::Double);
        heap.retain_code_owner(list);
        assert_eq!(audited_list_storage(&heap, list).unwrap().1, 0);
        for n in [-1, 4] {
            heap.set_field(list, 0, n, false);
            assert!(audited_list_storage(&heap, list).is_err());
        }
        heap.set_field(list, 0, 0, true);
        assert!(audited_list_storage(&heap, list).is_err());
        heap.set_field(list, 0, 0, false);
        for (data, mark) in [(fixed, false), (other, true), (list, true), (0, true)] {
            heap.set_field(list, 1, data, mark);
            assert!(audited_list_storage(&heap, list).is_err());
        }
        let fake = heap.allocate_instance(cid::GROWABLE_LIST, 1);
        assert!(audited_list_storage(&heap, fake).is_err());
    }

    #[test]
    fn owned_element_survives_cleared_slot_and_released_list() {
        for arc in [false, true] {
            for class in [cid::LIST, cid::IMMUTABLE_LIST, cid::GROWABLE_LIST] {
                let previous = HEAP.with(|h| h.replace(Heap::new(true)));
                if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
                let child = dartforge_arc_box_int_owned_v1(i64::MAX);
                let list = HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    let list = if class == cid::GROWABLE_LIST { heap.new_growable(3, 5, Element::General) }
                        else { heap.new_list(class, 3, Element::General) };
                    heap.retain_code_owner(list);
                    heap.list_set(list, 1, Value::Ref(child));
                    heap.list_set(list, 2, Value::Ref(smi::of(7).unwrap()));
                    list
                });
                dartforge_arc_release(child);
                let stats = HEAP.with(|h| h.borrow().stats());
                let received = dartforge_arc_list_ref_owned_v1(list, 1);
                assert_eq!(received, child);
                assert_eq!(dartforge_arc_list_ref_owned_v1(list, 0), 0);
                assert_eq!(dartforge_arc_list_ref_owned_v1(list, 2), smi::of(7).unwrap());
                HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    assert_eq!(heap.stats().allocations, stats.allocations);
                    assert_eq!(heap.stats().collections, stats.collections);
                    heap.list_set(list, 1, Value::Ref(0));
                });
                dartforge_arc_release(list);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observe_heap_v1(list), 0);
                assert_eq!(dartforge_arc_observe_heap_v1(received), 3);
                assert_eq!(dartforge_unbox_int(received), i64::MAX);
                dartforge_arc_release(received);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observe_heap_v1(received), 0);
                HEAP.with(|h| h.replace(previous));
            }
        }
    }

    #[test]
    fn read_rejects_fake_compacts_indices_and_layouts() {
        let mut heap = Heap::new(true);
        for shape in [Element::General, Element::Int, Element::Double, Element::Bool] {
            let list = heap.new_list(cid::LIST, 2, shape);
            for ix in [-1, 2, i64::MAX] { assert!(read_audited_ref_element(&heap, list, ix).is_err()); }
            assert_eq!(read_audited_ref_element(&heap, list, 0).is_ok(), shape == Element::General);
        }
        for receiver in [0, smi::of(3).unwrap(), i64::MAX] {
            assert!(read_audited_ref_element(&heap, receiver, 0).is_err());
        }
        let fake = heap.allocate_instance(cid::GROWABLE_LIST, 2);
        heap.set_field(fake, 0, 1, false);
        heap.set_field(fake, 1, 0, false);
        assert!(read_audited_ref_element(&heap, fake, 0).is_err());
        let list = heap.new_growable(1, 3, Element::General);
        assert!(read_audited_ref_element(&heap, list, 1).is_err());
        heap.set_field(list, 0, 4, false);
        assert!(read_audited_ref_element(&heap, list, 0).is_err());
    }
}

fn arc_argument_index(positional: i64, named: i64, ix: i64) -> Result<usize, &'static str> {
    let total = positional.checked_add(named).filter(|_| positional >= 0 && named >= 0)
        .and_then(|n| usize::try_from(n).ok()).filter(|n| *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .ok_or("invalid descriptor counts")?;
    usize::try_from(ix).ok().filter(|i| *i < total).ok_or("index outside the Ref arguments")
}

/// Reads a Ref argument of the uniform ABI and hands out an independent owner.
///
/// Checks the index against the sum of positionals and named ones: the later RTI
/// slot is not a Ref. Retains before returning, without allocating, collecting,
/// running Dart or changing the vectors. The caller must consume the token.
///
/// # Safety
/// `descriptor` must point to at least two aligned and immutable i64s.
/// `arguments` must contain at least the declared number of valid Refs,
/// aligned and sustained by the caller, without mutation during this call.
/// The index check does not prove the actual extent of external pointers.
///
/// # Panics
/// Null pointer, invalid count/index or internal retention failure;
/// aborts at the C boundary, without a Dart exception.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_object_owned_v1(123, 0);
/// let arguments = [object];
/// let descriptor = [1, 0];
/// // SAFETY: live and immutable vectors; object sustained by the caller.
/// let received = unsafe { dartforge_arc_argument_owned_v1(arguments.as_ptr(), descriptor.as_ptr(), 0) };
/// dartforge_arc_release(object);
/// assert_eq!(dartforge_arc_observe_heap_v1(received), 3);
/// dartforge_arc_release(received);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_argument_owned_v1(arguments: *const i64, descriptor: *const i64, ix: i64) -> i64 {
    assert!(!descriptor.is_null() && !arguments.is_null(), "null ARC ABI vectors");
    // SAFETY: two valid elements per the ABI contract.
    let ix = arc_argument_index(unsafe { *descriptor }, unsafe { *descriptor.add(1) }, ix)
        .expect("ARC argument outside the descriptor");
    // SAFETY: index checked and extent guaranteed by the caller.
    let value = unsafe { *arguments.add(ix) };
    HEAP.with(|heap| heap.borrow_mut().retain_code_owner(value));
    value
}

#[cfg(test)]
mod tests_owned_argument {
    use super::*;

    #[test]
    #[allow(unsafe_code)]
    fn ref_arguments_check_bounds_and_survive_caller_owner() {
        for (pos, named_arg, i) in [(-1, 0, 0), (0, -1, 0), (1, 0, -1), (1, 0, 1), (0, 0, 0), (i64::MAX, 1, 0)] {
            assert!(arc_argument_index(pos, named_arg, i).is_err());
        }
        assert_eq!(arc_argument_index(1, 2, 2), Ok(2));
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(crate::heap::Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = dartforge_arc_object_owned_v1(32001, 0);
            let arguments = [0, object, crate::layout::smi::of(7).unwrap(), i64::MAX];
            let descriptor = [1, 2, 101, 102];
            let stats = HEAP.with(|h| h.borrow().stats());
            // SAFETY: three valid arguments followed by an RTI that will not be read.
            let received = unsafe { dartforge_arc_argument_owned_v1(arguments.as_ptr(), descriptor.as_ptr(), 1) };
            assert_eq!(received, object);
            dartforge_arc_release(object);
            assert_eq!(dartforge_arc_observe_heap_v1(received), 3);
            HEAP.with(|h| { assert_eq!(h.borrow().stats().allocations, stats.allocations);
                assert_eq!(h.borrow().stats().collections, stats.collections); });
            dartforge_arc_release(received);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(received), 0);
            HEAP.with(|h| h.replace(previous));
        }
    }
}

/// Allocates a zeroed instance and hands exactly one Owned token to the caller.
///
/// The class must fit in i32 and the field count in u16, like the physical
/// header. All fields start with zero bits and no reference mark,
/// including in the extended map. Allocation may collect; retention happens
/// in the same heap borrow, before returning the handle or refilling the TLAB.
/// Does not run a Dart constructor, register methods or certify layout/reload.
/// The caller must transfer the token or release it exactly once.
///
/// # Panics
/// Invalid class/count or internal allocation/counting failure; aborts
/// at the C boundary. Does not represent a Dart exception.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_object_owned_v1(123, 40);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(object), 3);
/// assert_eq!(dartforge_arc_read_field_scalar_v1(object, 39), 0);
/// dartforge_arc_release(object);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(object), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_object_owned_v1(class: i64, fields: i64) -> i64 {
    let (class, fields) = owned_instance_parameters(class, fields)
        .expect("ARC instance allocation contract violated");
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let object = heap.allocate_instance(class, fields);
        heap.retain_code_owner(object);
        heap.refill_tlab(crate::layout::instance_words(fields));
        object
    })
}

/// Creates a closure with an independent owner and a borrowed context.
///
/// Code and typed body are native addresses of the image; their ABI and lifetime
/// remain the emitter's responsibility. Does not call those addresses.
/// The context gains the closure's edge without consuming the caller's token.
/// Initialization and retention happen in the same heap borrow. May
/// collect, but does not run Dart nor leave a pending Dart exception.
///
/// # Panics
/// Internal allocation/counting failure; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn input(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
/// let closure = dartforge_arc_closure_owned_v1(input as usize as i64, 0, 0, 0);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(closure), 3);
/// dartforge_arc_release(closure);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_closure_owned_v1(code: i64, context: i64, typed_one: i64, abi: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let closure = heap.new_closure(code, (context, true), typed_one, abi);
        heap.retain_code_owner(closure);
        closure
    })
}

/// Borrows the Ref context of a closure without retaining or collecting.
///
/// Checks the class and the physical map. A scalar context is only accepted when
/// it is zero, used by canonical closures without captures. The borrow
/// depends on the closure and its edge; it must be copied before invalidating it.
/// Does not run Dart nor leave a pending exception.
///
/// # Panics
/// Receiver is not a closure or non-null scalar context; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn input(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
/// let closure = dartforge_arc_closure_owned_v1(input as usize as i64, 0, 0, 0);
/// assert_eq!(dartforge_arc_closure_context_v1(closure), 0);
/// dartforge_arc_release(closure);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_closure_context_v1(closure: i64) -> i64 {
    HEAP.with(|heap| audited_closure_context(&heap.borrow(), closure)
        .expect("ARC closure context contract violated"))
}

/// Borrows a Ref capture of an environment, checking its physical map.
///
/// Requires a live environment of class CONTEXTO, a valid index and a field marked Ref.
/// The result depends on the environment's edge; it must be copied before
/// replacing the capture or releasing the environment. Does not retain, allocate, collect,
/// run Dart or change the pending exception.
///
/// # Panics
/// Invalid class, index or representation; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::{abi::*, layout::cid};
/// let env = dartforge_arc_object_owned_v1(i64::from(cid::CONTEXT), 1);
/// dartforge_arc_write_field_ref_v1(env, 0, 0);
/// assert_eq!(dartforge_arc_env_ref_v1(env, 0), 0);
/// dartforge_arc_release(env);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_env_ref_v1(env: i64, ix: i64) -> i64 {
    HEAP.with(|heap| audited_env_ref(&heap.borrow(), env, ix)
        .expect("ARC Ref capture contract violated"))
}

/// Copies the bits of a scalar capture of an environment, checking its map.
///
/// Requires a live environment of class CONTEXTO, a valid index and a field not marked
/// Ref. The bits follow the representation the capture was written with; the
/// conversion (`double`, `bool`) is up to the caller. Does not retain, allocate,
/// collect, run Dart or change the pending exception.
///
/// # Panics
/// Invalid class, index or representation; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::{abi::*, layout::cid};
/// let env = dartforge_arc_object_owned_v1(i64::from(cid::CONTEXT), 1);
/// dartforge_arc_write_field_scalar_v1(env, 0, 42);
/// assert_eq!(dartforge_arc_scalar_env_v1(env, 0), 42);
/// dartforge_arc_release(env);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_scalar_env_v1(env: i64, ix: i64) -> i64 {
    HEAP.with(|heap| audited_env_scalar(&heap.borrow(), env, ix)
        .expect("ARC scalar capture contract violated"))
}

fn audited_env_scalar(heap: &Heap, env: i64, ix: i64) -> Result<i64, &'static str> {
    let object = heap.object(env).ok_or("receiver is not a live environment")?;
    if object.class_id != i64::from(crate::layout::cid::CONTEXT) {
        return Err("receiver is not an environment");
    }
    let ix = usize::try_from(ix).map_err(|_| "negative index")?;
    let (bits, reference) = object.get(ix).ok_or("index outside the environment")?;
    if reference { return Err("capture is a managed reference"); }
    Ok(bits)
}

fn audited_env_ref(heap: &Heap, env: i64, ix: i64) -> Result<i64, &'static str> {
    let object = heap.object(env).ok_or("receiver is not a live environment")?;
    if object.class_id != i64::from(crate::layout::cid::CONTEXT) {
        return Err("receiver is not an environment");
    }
    let ix = usize::try_from(ix).map_err(|_| "negative index")?;
    let (bits, reference) = object.get(ix).ok_or("index outside the environment")?;
    if !reference { return Err("capture is scalar"); }
    Ok(bits)
}

fn audited_closure_context(heap: &Heap, closure: i64) -> Result<i64, &'static str> {
    let context = heap.closure(closure).ok_or("receiver is not a closure")?.context;
    if !context.1 && context.0 != 0 { return Err("context is a non-null scalar"); }
    Ok(context.0)
}

#[cfg(test)]
mod tests_closure_context {
    use super::*;

    #[test]
    fn env_checks_class_bounds_and_extended_map_without_collecting() {
        let mut heap = Heap::new(false);
        let child = heap.new_object(32000, &[]);
        let mut fields = vec![(0, false); 40];
        fields[0] = (0, true);
        fields[1] = (smi::of(42).unwrap(), true);
        fields[38] = (child, false);
        fields[39] = (child, true);
        let env = heap.new_object(i64::from(crate::layout::cid::CONTEXT), &fields);
        let false_value = heap.new_object(i64::from(crate::layout::cid::CELL), &fields);
        let stats = heap.stats();
        for (ix, expected) in [(0, 0), (1, fields[1].0), (39, child)] {
            assert_eq!(audited_env_ref(&heap, env, ix), Ok(expected));
        }
        for (receiver, ix) in [(env, -1), (env, 40), (env, 38), (env, 37),
            (0, 0), (false_value, 39), (child, 0)] {
            assert!(audited_env_ref(&heap, receiver, ix).is_err());
        }
        assert_eq!(heap.stats().allocations, stats.allocations);
        assert_eq!(heap.stats().collections, stats.collections);
    }

    #[test]
    fn context_checks_class_and_map_without_mistaking_bits_for_ref() {
        let mut heap = Heap::new(false);
        for field in [(0, false), (0, true), (42, false)] {
            let closure = heap.new_closure(123, field, 0, 0);
            assert_eq!(audited_closure_context(&heap, closure).is_ok(), field.0 == 0);
        }
        let child = heap.new_object(32000, &[]);
        let closure = heap.new_closure(123, (child, true), 0, 0);
        assert_eq!(audited_closure_context(&heap, closure), Ok(child));
        let fake = heap.new_object(32001, &[(123, false), (child, true), (0, false), (0, false)]);
        for invalid in [0, fake] { assert!(audited_closure_context(&heap, invalid).is_err()); }
    }
}

fn owned_instance_parameters(class: i64, fields: i64) -> Result<(i32, usize), &'static str> {
    let class = i32::try_from(class).map_err(|_| "class outside i32")?;
    let fields = u16::try_from(fields).map_err(|_| "field count outside u16")?;
    Ok((class, usize::from(fields)))
}

/// Registers the static method table and allocates a zeroed Owned instance.
///
/// The `table` function is the emitter's static thunk: it returns a valid table
/// `{cid, n, [hash, input]…}`, without running Dart, collecting or throwing. The memory
/// and the entries must stay valid according to the generations protocol.
/// Registration happens before allocation; the token has the same contract as
/// [`dartforge_arc_object_owned_v1`]. Does not run the Dart constructor.
///
/// # Safety
/// `table` must fulfill the protocol above: readable pointer, valid count and
/// entries, durable memory and absence of Dart/GC/exceptions. The registration
/// function reads the returned memory; an invalid pointer cannot be verified.
///
/// # Panics
/// Invalid class/count or internal failure; aborts at the C boundary. An invalid
/// table violates the native protocol, as in `dartforge_object_new_t`.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn table() -> *const i64 {
///     static TABLE: [i64; 2] = [32000, 0];
///     TABLE.as_ptr()
/// }
/// // SAFETY: pure thunk, valid static table without entries.
/// let object = unsafe { dartforge_arc_object_owned_t_v1(32000, 0, table) };
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(object), 3);
/// dartforge_arc_release(object);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(object), 0);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_object_owned_t_v1(class: i64, fields: i64, table: extern "C" fn() -> *const i64) -> i64 {
    owned_instance_parameters(class, fields).expect("ARC instance allocation contract violated");
    dartforge_register_table(class, table);
    dartforge_arc_object_owned_v1(class, fields)
}

/// Writes scalar bits into a field of a borrowed instance.
///
/// Requires a live and mutable receiver, a valid index and a scalar layout certified
/// by the lowering. Does not interpret the bits as a handle: removes the Ref mark and
/// the previous strong edge, even when the new bits look like an address.
/// Applies the heap barrier. Does not allocate, collect or run Dart; invalidates
/// borrows because the replacement may remove the child's last edge.
/// Does not do Dart type/late guards nor certify layout version or pin.
///
/// # Panics
/// Violated preconditions or internal counting failure; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_object_owned_v1(123, 40);
/// dartforge_arc_write_field_scalar_v1(object, 39, i64::MAX);
/// assert_eq!(dartforge_arc_read_field_scalar_v1(object, 39), i64::MAX);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_write_field_scalar_v1(object: i64, ix: i64, bits: i64) {
    HEAP.with(|heap| write_audited_field(&mut heap.borrow_mut(), object, ix, bits, false)
        .expect("ARC scalar write contract violated"));
}

/// Publishes a borrowed reference as a strong edge of an instance.
///
/// Does not consume a token of the receiver nor of the value. Requires a live and mutable receiver,
/// a valid index, a live Ref value (or null/Smi/permanent) and a certified Ref
/// layout. Updates bits, map and barrier, retaining the new edge before
/// releasing the previous one. Self-assignment does not change the multiplicity.
/// Does not allocate, collect or run Dart. Invalidates borrows through the edge
/// replacement; Dart type/late guards and version/pins are obligations of the lowering.
///
/// # Panics
/// Violated preconditions or internal counting failure; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_object_owned_v1(123, 40);
/// let child = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_write_field_ref_v1(object, 39, child);
/// dartforge_arc_release(child);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(child), 1);
/// dartforge_arc_write_field_ref_v1(object, 39, 0);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(child), 0);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_write_field_ref_v1(object: i64, ix: i64, value: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        write_audited_field(&mut heap, object, ix, value, true)
            .expect("ARC Ref write contract violated");
        heap.arc_settle();
    });
}

/// Publishes an owned reference as a strong edge of an instance, consuming the token.
///
/// The real ARC's move into a field: the value's code token becomes the field's
/// occurrence (the generated code emits no release for it), the previous edge is
/// released, and the receiver's preconditions are those of
/// [`dartforge_arc_write_field_ref_v1`]. Does not allocate, collect or run Dart.
///
/// # Panics
/// Violated preconditions or internal counting failure; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_object_owned_v1(123, 1);
/// let child = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_write_field_ref_move_v1(object, 0, child);
/// assert_eq!(dartforge_arc_read_field_ref_v1(object, 0), child);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_write_field_ref_move_v1(object: i64, ix: i64, value: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let view = heap.object(object).expect("ARC Ref write contract violated: receiver is not a live object");
        let ix = usize::try_from(ix).ok().filter(|&i| i < view.len())
            .expect("ARC Ref write contract violated: index outside the object");
        heap.set_field_moving(object, ix, value);
        heap.arc_settle();
    });
}

/// Writes a Ref edge only after checking the receiver's physical layout.
///
/// `layouts` contains sorted pairs `(cid, field_count)` of classes
/// whose field `ix` was declared Ref by the compiler. The check uses
/// the actual class and size, including on the first write of the zeroed field.
/// Does not consume owners, allocate, collect or run Dart; invalidates borrows
/// of the previous edge. Dart type/late guards remain in the lowering.
///
/// # Safety
/// The vector must be aligned and contain `pair_list * 2` readable words,
/// immutable during the call. Each pair must match the current layout
/// of the image, with `ix` Ref. The function neither keeps nor returns the address.
/// Lifetime/versions of the image are the caller's obligation.
///
/// # Panics
/// Receiver, index, class or size outside the layouts; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let layouts = [123, 40];
/// let object = dartforge_arc_object_owned_v1(123, 40);
/// let child = dartforge_arc_box_int_owned_v1(i64::MAX);
/// // SAFETY: live and immutable vector, field 39 declared Ref in this image.
/// unsafe { dartforge_arc_write_field_ref_checked_v1(object, 39, child, layouts.as_ptr(), 1); }
/// dartforge_arc_release(child);
/// assert_eq!(dartforge_arc_read_field_ref_v1(object, 39), child);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_arc_write_field_ref_checked_v1(
    object: i64, ix: i64, value: i64, layouts: *const i64, pair_list: i64,
) {
    let words = usize::try_from(pair_list).ok().and_then(|n| n.checked_mul(2))
        .filter(|n| *n > 0 && *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .expect("invalid ARC layouts vector");
    assert!(!layouts.is_null() && layouts.is_aligned(), "invalid ARC layouts address");
    // SAFETY: the producer guarantees the extent, immutability and lifetime of the vector.
    let layouts = unsafe { std::slice::from_raw_parts(layouts, words) };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        write_checked_ref_field(&mut heap, object, ix, value, layouts)
            .expect("ARC checked Ref write contract violated");
        heap.arc_settle();
    });
}

fn write_checked_ref_field(
    heap: &mut Heap, object: i64, ix: i64, value: i64, layouts: &[i64],
) -> Result<(), &'static str> {
    check_physical_layout(heap, object, ix, layouts)?;
    write_audited_field(heap, object, ix, value, true)
}

fn check_physical_layout(heap: &Heap, object: i64, ix: i64, layouts: &[i64]) -> Result<(), &'static str> {
    let view = heap.object(object).ok_or("receiver is not a live object")?;
    let i = usize::try_from(ix).map_err(|_| "negative index")?;
    if i >= view.len() { return Err("index outside the object"); }
    let fits = |fields: i64| usize::try_from(fields).ok() == Some(view.len());
    if layouts.len() % 2 != 0 || !(layout_fields(layouts, view.class_id).is_some_and(fits)
        || layouts.chunks_exact(2).any(|p| p[0] == view.class_id && fits(p[1]))) {
        return Err("class/size outside the Ref layouts");
    }
    Ok(())
}

/// The declared fields of `cid` in the pairs `(cid, fields)` in `cid` order
/// (the producer's, `layouts_do_campo`), by binary search. A vector out of
/// order only loses the shortcut: the caller checks again sequentially.
fn layout_fields(layouts: &[i64], cid: i64) -> Option<i64> {
    let (mut low, mut high) = (0, layouts.len() / 2);
    while low < high {
        let mid = (low + high) / 2;
        match layouts[2 * mid].cmp(&cid) {
            std::cmp::Ordering::Less => low = mid + 1,
            std::cmp::Ordering::Greater => high = mid,
            std::cmp::Ordering::Equal => return Some(layouts[2 * mid + 1]),
        }
    }
    None
}

/// Writes scalar bits after checking class and size against declared layouts.
///
/// `representation` is 1=int, 2=double, 3=bool, 4=byte. The bits must already be
/// converted by the lowering; does not coerce, allocate, collect or run Dart.
/// Keeps the barrier and invalidates earlier borrows when replacing the field.
///
/// # Safety
/// `layouts` must point to `pair_list * 2` aligned words, immutable and
/// readable during the call. The pairs `(cid, fields)` must declare the
/// index with the indicated representation in the current version of the image.
/// Neither keeps nor returns that address; versions/pins are the caller's duty.
///
/// # Panics
/// Invalid representation, receiver, index, class or size; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let layouts = [123, 1];
/// let object = dartforge_arc_object_owned_v1(123, 1);
/// // SAFETY: live vector; field 0 is int in the current layout of the image.
/// unsafe { dartforge_arc_write_field_scalar_checked_v1(object, 0, 42, layouts.as_ptr(), 1, 1); }
/// assert_eq!(dartforge_arc_read_field_scalar_v1(object, 0), 42);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_arc_write_field_scalar_checked_v1(
    object: i64, ix: i64, bits: i64, layouts: *const i64, pair_list: i64, representation: i64,
) {
    assert!((1..=4).contains(&representation), "invalid ARC scalar representation");
    let words = usize::try_from(pair_list).ok().and_then(|n| n.checked_mul(2))
        .filter(|n| *n > 0 && *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .expect("invalid ARC layouts vector");
    assert!(!layouts.is_null() && layouts.is_aligned(), "invalid ARC layouts address");
    // SAFETY: vector and extent guaranteed by the producer.
    let layouts = unsafe { std::slice::from_raw_parts(layouts, words) };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        check_physical_layout(&heap, object, ix, layouts).expect("incompatible ARC scalar layout");
        write_audited_field(&mut heap, object, ix, bits, false)
            .expect("ARC checked scalar write contract violated");
    });
}

/// Reads the scalar bits after checking the physical layout and the actual field map.
///
/// The representation (1=int, 2=double, 3=bool, 4=byte) and the conversions are
/// obligations of the lowering. Does not create owners, allocate, collect or run Dart.
///
/// # Safety
/// The `(cid, fields)` vector must contain `pair_list * 2` aligned words,
/// immutable and readable. Each pair must declare the index with the representation
/// indicated in the current layout of the image. Does not keep that address.
/// Lifetime/versions and Dart type/late guards are the caller's responsibility.
///
/// # Panics
/// Incompatible representation, receiver, index, class, size or map;
/// aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let layouts = [123, 1];
/// let object = dartforge_arc_object_owned_v1(123, 1);
/// dartforge_arc_write_field_scalar_v1(object, 0, 42);
/// // SAFETY: live vector; field 0 is int in the current layout of the image.
/// let bits = unsafe { dartforge_arc_read_field_scalar_checked_v1(object, 0, layouts.as_ptr(), 1, 1) };
/// assert_eq!(bits, 42);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_arc_read_field_scalar_checked_v1(
    object: i64, ix: i64, layouts: *const i64, pair_list: i64, representation: i64,
) -> i64 {
    assert!((1..=4).contains(&representation), "invalid ARC scalar representation");
    let words = usize::try_from(pair_list).ok().and_then(|n| n.checked_mul(2))
        .filter(|n| *n > 0 && *n <= isize::MAX as usize / std::mem::size_of::<i64>())
        .expect("invalid ARC layouts vector");
    assert!(!layouts.is_null() && layouts.is_aligned(), "invalid ARC layouts address");
    // SAFETY: vector and extent guaranteed by the producer.
    let layouts = unsafe { std::slice::from_raw_parts(layouts, words) };
    HEAP.with(|heap| {
        let heap = heap.borrow();
        check_physical_layout(&heap, object, ix, layouts).expect("incompatible ARC scalar layout");
        read_audited_scalar_field(&heap, object, ix)
            .expect("ARC checked scalar read contract violated")
    })
}

fn write_audited_field(heap: &mut Heap, object: i64, ix: i64, bits: i64, reference: bool) -> Result<(), &'static str> {
    let view = heap.object(object).ok_or("receiver is not a live object")?;
    let ix = usize::try_from(ix).map_err(|_| "negative index")?;
    if ix >= view.len() { return Err("index outside the object"); }
    heap.set_field(object, ix, bits, reference);
    Ok(())
}

/// Copies the bits of a scalar field of a borrowed object.
///
/// Requires a live object, a valid index and a field not marked as a reference in
/// the heap map. The lowering must prove the matching layout before
/// choosing this ABI. Does not allocate, collect, retain the object, invalidate borrows
/// nor convert the bits. The catalog may declare a scalar result because
/// the implementation checks the map, not because the representation is i64.
///
/// # Panics
/// Violated layout/object/index preconditions; aborts at the C boundary.
/// Does not replace the Dart checks of receiver, type or late initialization.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_object_new(123, 1);
/// dartforge_arc_retain(object);
/// dartforge_object_set(object, 0, i64::MAX, 0);
/// assert_eq!(dartforge_arc_read_field_scalar_v1(object, 0), i64::MAX);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_read_field_scalar_v1(object: i64, ix: i64) -> i64 {
    HEAP.with(|heap| read_audited_scalar_field(&heap.borrow(), object, ix)
        .expect("ARC scalar read contract violated"))
}

fn read_audited_scalar_field(heap: &Heap, object: i64, ix: i64) -> Result<i64, &'static str> {
    // The runtime's positional record: word 0 is the number of fields.
    if let Some(fields) = heap.record(object) {
        return if ix == 0 { Ok(fields.len() as i64) } else { Err("record field is a reference") };
    }
    let object = heap.readable_object(object).ok_or("receiver is not a live object")?;
    let ix = usize::try_from(ix).map_err(|_| "negative index")?;
    let (bits, reference) = object.get(ix).ok_or("index outside the object")?;
    if reference { return Err("field is a managed reference"); }
    Ok(bits)
}

/// Borrows a field reference, without creating an Owned token for the result.
///
/// Requires a live instance, a valid index and a Ref mark in the actual heap map.
/// Null/Smi are valid references. The result depends on the receiver's edge:
/// keeping only the receiver alive does not protect the child after replacing the field.
/// The actual map checks the Ref representation even without a static nominal class.
/// The lowering must preserve semantic guards and borrow limits; before
/// an invalidation, it must copy to Owned or end its use.
/// Does not allocate, collect, retain, invalidate other borrows or run Dart.
/// Does not replace Dart type/late guards nor certify versions/pins.
///
/// # Panics
/// Incompatible receiver/index/map; internal failure aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_object_owned_v1(123, 40);
/// let child = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_write_field_ref_v1(object, 39, child);
/// dartforge_arc_release(child);
/// let borrowed = dartforge_arc_read_field_ref_v1(object, 39);
/// assert_eq!(borrowed, child);
/// dartforge_arc_retain(borrowed); // Copies the borrow before the replacement.
/// dartforge_arc_write_field_ref_v1(object, 39, 0);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(borrowed), 3);
/// dartforge_arc_release(borrowed);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_read_field_ref_v1(object: i64, ix: i64) -> i64 {
    HEAP.with(|heap| read_audited_ref_field(&heap.borrow(), object, ix)
        .expect("ARC Ref read contract violated"))
}

fn read_audited_ref_field(heap: &Heap, object: i64, ix: i64) -> Result<i64, &'static str> {
    // The runtime's positional record: fields `1..=n` are references.
    if let Some(fields) = heap.record(object) {
        return usize::try_from(ix).ok().filter(|&i| i >= 1)
            .and_then(|i| fields.get(i - 1).copied())
            .ok_or("index outside the record");
    }
    let object = heap.readable_object(object).ok_or("receiver is not a live object")?;
    let ix = usize::try_from(ix).map_err(|_| "negative index")?;
    let (bits, reference) = object.get(ix).ok_or("index outside the object")?;
    // The field not yet written is born zeroed and marked scalar: it is the null of the
    // Ref field without an initializer (`Object? x;`), with no object nor owner.
    if !reference && bits != 0 { return Err("field is scalar"); }
    Ok(bits)
}

/// Reads a Ref borrow of a late field or the null before initialization.
///
/// Requires a live receiver and a valid index. A field marked Ref follows the same
/// edge contract as the Ref reader. Without that mark, only the zero of a field
/// not yet initialized can come out as null; scalar bits do not become Ref.
/// Does not mark initialization, retain, allocate, collect or run Dart. Late guards,
/// initializers and copying before invalidating the edge remain with the caller.
///
/// # Panics
/// Incompatible receiver, index or representation; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let object = dartforge_arc_object_owned_v1(123, 1);
/// assert_eq!(dartforge_arc_read_field_late_ref_v1(object, 0), 0);
/// dartforge_arc_write_field_ref_v1(object, 0, 0);
/// dartforge_late_field_mark_initialized(object, 0);
/// assert_eq!(dartforge_arc_read_field_late_ref_v1(object, 0), 0);
/// dartforge_arc_release(object);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_read_field_late_ref_v1(object: i64, ix: i64) -> i64 {
    HEAP.with(|heap| read_audited_late_ref_field(&heap.borrow(), object, ix)
        .expect("ARC late Ref read contract violated"))
}

fn read_audited_late_ref_field(heap: &Heap, object: i64, ix: i64) -> Result<i64, &'static str> {
    let view = heap.object(object).ok_or("receiver is not a live object")?;
    let index = usize::try_from(ix).map_err(|_| "negative index")?;
    let (bits, reference) = view.get(index).ok_or("index outside the object")?;
    if reference || (bits == 0 && !heap.late_initialized((object, ix))) {
        Ok(bits)
    } else {
        Err("late field holds neither a Ref nor the zero before initialization")
    }
}

/// Throws a borrowed Ref value and keeps it as the root of the pending exception.
///
/// Does not consume the caller's token. The exception protocol keeps an independent root
/// until clear/consumption; preparing the stack trace may allocate/collect.
/// The lowering must provide a live Ref and check for the pending exception immediately.
/// Does not convert scalar bits nor apply the Dart semantics of `throw null`.
///
/// # Panics
/// Internal failure of the exception/heap protocol; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_box_int_owned_v1, dartforge_arc_throw_ref_v1,
///     dartforge_arc_release, dartforge_exception_pending, dartforge_exception_clear};
/// let value = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_throw_ref_v1(value);
/// assert_ne!(dartforge_exception_pending(), 0);
/// dartforge_arc_release(value);
/// dartforge_exception_clear();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_throw_ref_v1(value: i64) {
    dartforge_exception_throw(value, 3);
}

/// Copies the pending exception into an Owned token independent of clear.
///
/// Preserves the identity of Ref values. Scalars are boxed within
/// the same heap borrow that publishes the owner, before returning the handle.
/// Without a pending exception returns null; releasing null does not change counters.
/// Does not consume the pending exception nor modify its stack trace. May allocate/collect.
///
/// # Panics
/// Internal allocation/ownership failure; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let original = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_throw_ref_v1(original);
/// let captured = dartforge_arc_exception_owned_v1();
/// assert_eq!(captured, original);
/// dartforge_arc_release(original);
/// dartforge_exception_clear();
/// dartforge_arc_collect();
/// dartforge_arc_release(captured);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_exception_owned_v1() -> i64 {
    let Some(value) = EXCEPTION.with(|slot| *slot.borrow()) else { return 0; };
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let reference = heap.to_ref(value);
        heap.retain_code_owner(reference);
        reference
    })
}

/// Copies the current stack trace into an Owned token independent of clear.
///
/// Without a current stack trace, creates a non-empty StackTrace, without installing it as
/// the stack trace of a future exception. May allocate/collect; publishes the owner before
/// returning the handle, without a safepoint between the read and the retention.
///
/// # Panics
/// Internal allocation/ownership failure; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let trace = dartforge_arc_stack_trace_owned_v1();
/// dartforge_arc_collect();
/// dartforge_arc_release(trace);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_stack_trace_owned_v1() -> i64 {
    let trace = dartforge_stack_trace_get();
    HEAP.with(|h| h.borrow_mut().retain_code_owner(trace));
    trace
}

/// Throws a borrowed Ref with an explicit borrowed StackTrace.
///
/// Does not consume the caller's two tokens. Publishes independent roots
/// for value and stack trace until clear/consumption. Requires both alive and a valid
/// StackTrace; does not apply the Dart semantics of `throw null`.
/// The lowering must check for the pending exception immediately after the call.
///
/// # Panics
/// Internal failure of the exception/heap protocol; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let value = dartforge_arc_box_int_owned_v1(i64::MAX);
/// let trace = dartforge_arc_stack_trace_owned_v1();
/// dartforge_arc_throw_with_stack_trace_ref_v1(value, trace);
/// let copy = dartforge_arc_stack_trace_owned_v1();
/// assert_eq!(copy, trace);
/// dartforge_arc_release(value);
/// dartforge_arc_release(trace);
/// dartforge_exception_clear();
/// dartforge_arc_collect();
/// dartforge_arc_release(copy);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_throw_with_stack_trace_ref_v1(value: i64, trace: i64) {
    dartforge_throw_with_stack_trace(value, 3, trace);
}

/// Hands out an int in Ref representation with an Owned token for the caller.
///
/// A Smi has no counter; values outside its range create a mortal Mint.
/// Allocation may collect, but the token is published before returning the handle.
/// The caller must transfer it or call release exactly once.
///
/// # Panics
/// Internal allocation/counting failure; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_box_int_owned_v1, dartforge_arc_release, dartforge_arc_collect};
/// let value = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_release(value);
/// dartforge_arc_collect();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_box_int_owned_v1(value: i64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let boxed = heap.box_int(value);
        heap.retain_code_owner(boxed);
        boxed
    })
}

/// Boxes a double and hands out an Owned token before any other collection.
///
/// Preserves all bits, including negative zero and NaN payload. The box
/// belongs to the caller until release or transfer to a slot/return.
///
/// # Panics
/// Reentrant heap borrow or allocation failure; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let value = dartforge_arc_box_double_owned_v1(-0.0);
/// assert_eq!(dartforge_arc_observe_heap_v1(value), 3);
/// dartforge_arc_release(value);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(value), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_box_double_owned_v1(value: f64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let boxed = heap.box_double(value);
        heap.retain_code_owner(boxed);
        boxed
    })
}

/// Boxes the 128 bits of a SIMD value and hands out a code owner.
///
/// The words preserve the native bits of the lanes, including NaN payload.
/// `cid` must identify Float32x4, Int32x4 or Float64x2. No lane is a Ref.
///
/// # Panics
/// Incompatible class, reentrant borrow or allocation failure; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::{abi::*, layout::cid};
/// let value = dartforge_arc_box_simd_owned_v1(cid::FLOAT32X4 as i64, 0, 0);
/// assert_eq!(dartforge_arc_observe_heap_v1(value), 3);
/// dartforge_arc_release(value);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(value), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_box_simd_owned_v1(cid: i64, low: i64, high: i64) -> i64 {
    let cid = i32::try_from(cid).expect("SIMD class outside the representation");
    assert!(matches!(cid, crate::layout::cid::FLOAT32X4 | crate::layout::cid::INT32X4 | crate::layout::cid::FLOAT64X2), "incompatible SIMD class");
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&low.to_ne_bytes());
    bytes[8..].copy_from_slice(&high.to_ne_bytes());
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let boxed = heap.new_simd(cid, bytes);
        heap.retain_code_owner(boxed);
        boxed
    })
}

#[cfg(test)]
mod tests_owned_simd_boxes {
    use super::*;

    #[test]
    fn hints_keep_bits_and_owner_protects_until_release_in_both_modes() {
        for arc in [false, true] {
            for cid in [crate::layout::cid::FLOAT32X4, crate::layout::cid::INT32X4,
                crate::layout::cid::FLOAT64X2] {
                HEAP.with(|h| h.replace(Heap::new(true)));
                if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
                for (low, high) in [(0i64, 0i64), (i64::MIN, 0x7ff8_0000_0000_0123),
                    (-1, i64::MAX), (0x7fc0_0123_8000_0000, 0x0000_0001_7f80_0000)] {
                    let h = dartforge_arc_box_simd_owned_v1(cid as i64, low, high);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observe_heap_v1(h), 3);
                    HEAP.with(|heap| {
                        let heap = heap.borrow();
                        assert_eq!(heap.header(h).class_id, cid);
                        let b = heap.simd(h).unwrap();
                        assert_eq!(&b[..8], &low.to_ne_bytes());
                        assert_eq!(&b[8..], &high.to_ne_bytes());
                    });
                    dartforge_arc_release(h);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observe_heap_v1(h), 0);
                }
            }
        }
    }
}

/// Observes a live block and a code owner without acquiring a reference to the object.
///
/// The argument is a native diagnostic address, not a borrowed Ref:
/// it may designate an already released handle. Bit 0 indicates a live block of this heap;
/// bit 1 indicates at least one code token. Null, Smi, static objects and
/// addresses outside the registered pages do not count as heap blocks.
/// Does not collect nor run Dart. The snapshot does not guarantee future validity;
/// reuse of the address may make a later observation see another object.
///
/// # Panics
/// Reentrant heap borrow; internal failure aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let address = dartforge_arc_box_int_owned_v1(i64::MAX);
/// assert_eq!(dartforge_arc_observe_heap_v1(address), 3);
/// dartforge_arc_release(address);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(address), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_observe_heap_v1(address: i64) -> u8 {
    HEAP.with(|h| h.borrow().observe_code_owner(address))
}

/// Creates an owning token of the generated code from a borrow.
///
/// Null and Smi have no counter. Does not collect nor run Dart. The token stays
/// in the heap inventory until release; owning slots are independent.
///
/// # Panics
/// Dead handle or overflow; internal failure aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_retain, dartforge_arc_release};
/// dartforge_arc_retain(0);
/// dartforge_arc_release(0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_retain(value: i64) {
    HEAP.with(|h| h.borrow_mut().retain_code_owner(value));
}

/// Consumes a token of the generated code, without collecting or running Dart.
///
/// Does not consume owners of frames, fields or messages. Null/Smi are no-ops.
/// RC reaching zero only schedules work for a later safepoint.
///
/// # Panics
/// Handle without a matching token; internal failure aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_retain, dartforge_arc_release};
/// dartforge_arc_retain(0);
/// dartforge_arc_release(0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_release(value: i64) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.release_code_owner(value);
        // The real ARC: the object whose last owner this was dies here.
        h.arc_settle();
    });
}

/// Registers the real ARC's drop functions of a program (`llvm/arc_tag_guards.rs`,
/// `drop_function`): `n` triples `(class, fields, function)`, the function
/// `i64 f(i64 handle, ptr out)` releasing the reference fields of a dying instance
/// of that class and field count. Neither allocates, collects nor runs Dart.
///
/// # Safety
/// `table` points to `3·n` readable words; each function follows the contract above
/// and lives as long as the image.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_arc_register_drops_v1(table: *const i64, n: i64) {
    let n = usize::try_from(n).expect("drop table length");
    // SAFETY: the function contract.
    let entries = unsafe { std::slice::from_raw_parts(table, 3 * n) };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        for e in entries.chunks_exact(3) {
            heap.register_drop(e[0], e[1], e[2]);
        }
    });
}

/// Runs an explicit collection at a safepoint with the roots already published.
///
/// May run native finalization callbacks; Dart callbacks are
/// queued, without running in this call. Under tracing it does a full collection.
///
/// # Panics
/// Internal collection failure; aborts at the C boundary.
///
/// ```
/// dartforge_runtime::abi::dartforge_arc_collect();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_collect() {
    HEAP.with(|h| h.borrow_mut().collect());
}

/// Checks the token version of this ABI and the ARC mode of the current heap.
///
/// Returns 1 only for version 1 with ARC active, 0 for incompatibility.
/// The caller must refuse ARC execution when the result is zero.
/// Does not certify the contracts of a module nor the return ABI of the externs.
///
/// ```
/// assert_eq!(dartforge_runtime::abi::dartforge_arc_verify_abi(-1), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_verify_abi(version: i64) -> i8 {
    i8::from(version == 1 && HEAP.with(|h| h.borrow().arc_active()))
}

/// Opens an owning frame with initially null slots.
///
/// # Panics
/// Negative count/incompatible with usize or internal failure; the C boundary
/// aborts, without unwinding into the generated code.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_frame_open_v1, dartforge_arc_frame_close_v1};
/// let frame = dartforge_arc_frame_open_v1(0);
/// dartforge_arc_frame_close_v1(frame);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_frame_open_v1(slots: i64) -> i64 {
    let slots = usize::try_from(slots).expect("invalid ARC slot count");
    HEAP.with(|h| h.borrow_mut().push_owning_frame(slots))
}

/// Copies the value into a slot, retaining before releasing the old content.
///
/// The frame and index are scalars; the value is a managed Ref, including null/Smi.
/// Zero consumes the previous content. Does not serve collection debt.
///
/// # Panics
/// Invalid frame/slot or dead reference; internal failure aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let frame = dartforge_arc_frame_open_v1(1);
/// dartforge_arc_frame_copy_v1(frame, 0, 0);
/// dartforge_arc_frame_close_v1(frame);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_frame_copy_v1(frame: i64, slot: i64, value: i64) {
    let slot = usize::try_from(slot).expect("invalid ARC slot index");
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.check_owning_frame(frame);
        h.set_root(frame, slot, value);
        h.arc_settle();
    });
}

/// Loads an owning slot and creates an owned code token.
///
/// The slot keeps its occurrence. The result needs release or transfer,
/// even if the frame is closed first. Does not collect nor run Dart.
///
/// # Panics
/// Observational/nonexistent frame, invalid slot or internal failure; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let frame = dartforge_arc_frame_open_v1(1);
/// let value = dartforge_arc_frame_load_v1(frame, 0);
/// dartforge_arc_frame_close_v1(frame);
/// dartforge_arc_release(value);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_frame_load_v1(frame: i64, slot: i64) -> i64 {
    let slot = usize::try_from(slot).expect("invalid ARC slot index");
    HEAP.with(|h| h.borrow_mut().copy_root_to_code(frame, slot))
}

/// Transfers a code token into an owning slot.
///
/// Does not retain the source. Publishes the destination before releasing its previous owner.
/// Null/Smi require no physical token. Does not collect nor run Dart.
///
/// # Panics
/// Invalid frame/slot or handle without a code token; validates before the mutation.
/// Internal failure aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let frame = dartforge_arc_frame_open_v1(1);
/// dartforge_arc_frame_receive_v1(frame, 0, 0);
/// dartforge_arc_frame_close_v1(frame);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_frame_receive_v1(frame: i64, slot: i64, value: i64) {
    let slot = usize::try_from(slot).expect("invalid ARC slot index");
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.move_code_to_root(frame, slot, value);
        h.arc_settle();
    });
}

/// Transfers a code token to the owner registry of a global.
///
/// `id` identifies the storage, it is not dereferenced. The generated code
/// must publish the actual value first, with no safepoint until this call. The registry
/// keeps only owners of handles; it does not provide the value of a global load.
/// Does not retain the source, collect or run Dart. Null/Smi remove the previous owner.
///
/// # Panics
/// Dead handle or without a code token; internal failure aborts at the C boundary.
///
/// ```
/// dartforge_runtime::abi::dartforge_arc_global_receive_v1(17, 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_global_receive_v1(id: i64, value: i64) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.move_code_to_global(id, value);
        h.arc_settle();
    });
}

/// Moves an owner between slots, consuming the source and the old content of the destination.
///
/// Moving to the same slot preserves the occurrence. There is no collection nor Dart
/// between publishing the destination and clearing the source.
///
/// # Panics
/// Non-owning or nonexistent frames or invalid indices; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let frame = dartforge_arc_frame_open_v1(2);
/// dartforge_arc_frame_move_v1(frame, 0, frame, 1);
/// dartforge_arc_frame_close_v1(frame);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_frame_move_v1(
    origin: i64,
    source_slot: i64,
    dest: i64,
    dest_slot: i64,
) {
    let of = usize::try_from(source_slot).expect("invalid ARC source index");
    let to = usize::try_from(dest_slot).expect("invalid ARC destination index");
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.move_root(origin, of, dest, to);
        h.arc_settle();
    });
}

/// Consumes the owners of the top frame without collecting or running Dart.
///
/// # Panics
/// The frame is not on top; internal failure aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let frame = dartforge_arc_frame_open_v1(1);
/// dartforge_arc_frame_close_v1(frame);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_frame_close_v1(frame: i64) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.check_owning_frame(frame);
        h.pop_frame(frame);
        h.arc_settle();
    });
}

#[cfg(test)]
mod tests_arc_abi_frames {
    use super::*;

    #[test]
    fn exception_ref_preserves_mint_after_release_and_clear_removes_root() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let value = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_throw_ref_v1(value);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_arc_release(value);
            dartforge_arc_collect();
            HEAP.with(|h| assert!(h.borrow().is_live_object(value)));
            assert_eq!(dartforge_exception_peek_ref(), value);
            dartforge_exception_clear();
            assert_eq!(dartforge_exception_pending(), 0);
            dartforge_arc_collect();
            HEAP.with(|h| assert!(!h.borrow().is_live_object(value)));
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn owned_capture_preserves_identity_and_survives_clear_in_both_modes() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            dartforge_exception_clear();
            assert_eq!(dartforge_arc_exception_owned_v1(), 0);
            let original = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_throw_ref_v1(original);
            let captured = dartforge_arc_exception_owned_v1();
            assert_eq!(captured, original);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_arc_release(original);
            dartforge_exception_clear();
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(captured)));
            assert_eq!(HEAP.with(|h| h.borrow().int_of(captured)), Some(i64::MAX));
            dartforge_arc_release(captured);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(captured)));
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn owned_trace_preserves_identity_and_dies_after_last_release() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            dartforge_exception_clear();
            let original = dartforge_arc_box_int_owned_v1(i64::MAX);
            let trace = dartforge_arc_stack_trace_owned_v1();
            assert!(CURRENT_STACK_TRACE.with(|slot| slot.borrow().is_none()));
            dartforge_arc_throw_with_stack_trace_ref_v1(original, trace);
            let captured_value = dartforge_arc_stack_trace_owned_v1();
            let value = dartforge_arc_exception_owned_v1();
            assert_eq!(captured_value, trace);
            assert_eq!(value, original);
            assert_eq!(CURRENT_STACK_TRACE.with(|slot| *slot.borrow()), Some(trace));
            dartforge_arc_release(original);
            dartforge_arc_release(trace);
            dartforge_exception_clear();
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(captured_value)));
            assert!(!trace_text(captured_value).unwrap().is_empty());
            assert_eq!(HEAP.with(|h| h.borrow().int_of(value)), Some(i64::MAX));
            dartforge_arc_release(captured_value);
            dartforge_arc_release(value);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(captured_value)));
            assert!(!HEAP.with(|h| h.borrow().is_live_object(value)));
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn owned_capture_boxes_scalar_without_consuming_pending() {
        use crate::heap::Value;
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            for (bits, tag, expected) in [
                (i64::MAX, 1, Value::Int(i64::MAX)),
                (42, 1, Value::Int(42)),
                (3.5_f64.to_bits() as i64, 4, Value::Double(3.5)),
                (1, 2, Value::Bool(true)),
                (0, 2, Value::Bool(false)),
                (0, 3, Value::Ref(0)),
            ] {
                dartforge_exception_throw(bits, tag);
                let captured = dartforge_arc_exception_owned_v1();
                assert_eq!(EXCEPTION.with(|slot| *slot.borrow()), Some(expected));
                dartforge_exception_clear();
                dartforge_arc_collect();
                assert_eq!(HEAP.with(|h| h.borrow().value(captured)), expected);
                dartforge_arc_release(captured);
                dartforge_arc_collect();
                if matches!(expected, Value::Int(i64::MAX) | Value::Double(_)) {
                    assert!(!HEAP.with(|h| h.borrow().is_live_object(captured)));
                }
            }
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn native_observation_creates_no_owner_and_rejects_addresses_outside_heap() {
        for validate in [false, true] {
            for arc in [false, true] {
                let previous = HEAP.with(|h| h.replace(Heap::new(validate)));
                if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
                for address in [0, 2, i64::MIN, i64::MAX, smi::of(42).unwrap()] {
                    assert_eq!(dartforge_arc_observe_heap_v1(address), 0);
                }
                let address = dartforge_arc_box_int_owned_v1(i64::MAX);
                assert_eq!(dartforge_arc_observe_heap_v1(address), 3);
                dartforge_arc_retain(address);
                dartforge_arc_release(address);
                assert_eq!(dartforge_arc_observe_heap_v1(address), 3);
                dartforge_arc_release(address);
                assert_eq!(dartforge_arc_observe_heap_v1(address), 1);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observe_heap_v1(address), 0);
                HEAP.with(|h| { h.replace(previous); });
            }
        }
    }

    #[test]
    fn owned_factory_hands_out_token_and_mint_dies_after_last_release() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let value = dartforge_arc_box_int_owned_v1(i64::MAX);
            assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
            assert_eq!(HEAP.with(|h| h.borrow().int_of(value)), Some(i64::MAX));
            // The copy represents a retained return, independent of the initial token.
            dartforge_arc_retain(value);
            dartforge_arc_release(value);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
            dartforge_arc_release(value);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(value)));
            let immediate = dartforge_arc_box_int_owned_v1(42);
            assert_eq!(immediate, smi::of(42).unwrap());
            dartforge_arc_release(immediate);
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn abi_global_receives_mortal_object_and_releases_last_owner() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc {
                HEAP.with(|h| h.borrow_mut().enable_arc());
            }
            // The address represents caller storage, without an observational
            // root that could hide the loss of the owning owner.
            let mut global = 0_i64;
            let id = (&mut global as *mut i64) as i64;
            let frame = dartforge_arc_frame_open_v1(1);
            let value = HEAP.with(|h| h.borrow_mut().allocate_str("global mortal"));
            dartforge_arc_frame_copy_v1(frame, 0, value);
            global = dartforge_arc_frame_load_v1(frame, 0);
            dartforge_arc_global_receive_v1(id, global);
            dartforge_arc_frame_close_v1(frame);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(global)));

            // Reassigning the same object consumes the new token and the old owner;
            // identity cannot skip the transfer or leak a retain.
            dartforge_arc_retain(global);
            dartforge_arc_global_receive_v1(id, global);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(value)));

            dartforge_arc_retain(global);
            global = smi::of(42).unwrap();
            dartforge_arc_global_receive_v1(id, global);
            dartforge_arc_collect();
            assert_eq!(global, smi::of(42).unwrap());
            assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
            dartforge_arc_release(value);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(value)));
            global = 0;
            dartforge_arc_global_receive_v1(id, global);
            HEAP.with(|h| {
                h.replace(previous);
            });
        }
    }

    #[test]
    fn abi_loads_independent_owner_and_receives_token_in_slot() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(false)));
            if arc {
                HEAP.with(|h| h.borrow_mut().enable_arc());
            }
            let frame = dartforge_arc_frame_open_v1(1);
            let value = HEAP.with(|h| h.borrow_mut().allocate_str("slot e SSA"));
            dartforge_arc_frame_copy_v1(frame, 0, value);
            let token = dartforge_arc_frame_load_v1(frame, 0);
            assert_eq!(token, value);
            dartforge_arc_frame_receive_v1(frame, 0, token);
            let token = dartforge_arc_frame_load_v1(frame, 0);
            dartforge_arc_frame_close_v1(frame);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(token)));
            dartforge_arc_release(token);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(token)));
            let frame = dartforge_arc_frame_open_v1(1);
            for scalar in [0, smi::of(42).unwrap()] {
                dartforge_arc_frame_receive_v1(frame, 0, scalar);
                assert_eq!(dartforge_arc_frame_load_v1(frame, 0), scalar);
                dartforge_arc_release(scalar);
            }
            dartforge_arc_frame_close_v1(frame);
            HEAP.with(|h| {
                h.replace(previous);
            });
        }
    }

    #[test]
    fn minimal_abi_checks_mode_and_keeps_tokens_until_release() {
        let previous = HEAP.with(|h| h.replace(Heap::new(false)));
        assert_eq!(dartforge_arc_verify_abi(1), 0);
        HEAP.with(|h| h.borrow_mut().enable_arc());
        assert_eq!(dartforge_arc_verify_abi(1), 1);
        assert_eq!(dartforge_arc_verify_abi(2), 0);
        let value = HEAP.with(|h| h.borrow_mut().allocate_str("token ABI"));
        dartforge_arc_retain(value);
        dartforge_arc_retain(value);
        dartforge_arc_collect();
        assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
        dartforge_arc_release(value);
        dartforge_arc_collect();
        assert!(HEAP.with(|h| h.borrow().is_live_object(value)));
        dartforge_arc_release(value);
        dartforge_arc_collect();
        assert!(!HEAP.with(|h| h.borrow().is_live_object(value)));
        HEAP.with(|h| {
            h.replace(previous);
        });
    }

    #[test]
    #[should_panic(expected = "ARC frame must be owning")]
    fn abi_rejects_observational_frame_before_changing_slots() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        heap.check_owning_frame(frame);
    }

    #[test]
    fn copy_move_and_closure_preserve_occurrences_until_last_owner() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(false)));
            if arc {
                HEAP.with(|h| h.borrow_mut().enable_arc());
            }
            let origin = dartforge_arc_frame_open_v1(2);
            let dest = dartforge_arc_frame_open_v1(1);
            let value = HEAP.with(|h| h.borrow_mut().allocate_str("owner ABI"));
            dartforge_arc_frame_copy_v1(origin, 0, value);
            dartforge_arc_frame_copy_v1(origin, 1, value);
            dartforge_arc_frame_move_v1(origin, 0, dest, 0);
            dartforge_arc_frame_move_v1(dest, 0, dest, 0);
            dartforge_arc_frame_copy_v1(origin, 1, 0);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert!(h.is_live_object(value));
            });
            dartforge_arc_frame_close_v1(dest);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert!(!h.is_live_object(value));
            });
            dartforge_arc_frame_close_v1(origin);
            HEAP.with(|h| {
                h.replace(previous);
            });
        }
    }
}

#[cfg(test)]
mod tests_owned_double_box {
    use super::*;
    #[test]
    fn owned_double_preserves_bits_and_dies_after_last_token() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            for bits in [(-0.0_f64).to_bits(), 3.25_f64.to_bits(), f64::INFINITY.to_bits(), 0x7ff80000deadbeef] {
                let value = dartforge_arc_box_double_owned_v1(f64::from_bits(bits));
                dartforge_arc_retain(value);
                dartforge_arc_release(value);
                dartforge_arc_collect();
                assert_eq!(HEAP.with(|h| h.borrow().double_of(value).unwrap().to_bits()), bits);
                assert_eq!(dartforge_arc_observe_heap_v1(value), 3);
                dartforge_arc_release(value);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observe_heap_v1(value), 0);
            }
            HEAP.with(|h| { h.replace(previous); });
        }
    }
}

#[cfg(test)]
mod field_ref_tests {
    use super::*;

    #[test]
    fn layouts_in_order_for_binary_search() {
        let layouts = [3, 2, 7, 4, 40, 1, 900, 6];
        for (cid, fields) in [(3, 2), (7, 4), (40, 1), (900, 6)] {
            assert_eq!(layout_fields(&layouts, cid), Some(fields));
        }
        assert_eq!(layout_fields(&layouts, 8), None);
        assert_eq!(layout_fields(&[], 8), None);
        // Out of order it may not find it; the check redoes it sequentially.
        assert_eq!(layout_fields(&[900, 6, 3, 2], 900), None);
    }

    #[test]
    fn positional_record_reads_scalar_count_and_ref_fields() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let child = dartforge_arc_box_int_owned_v1(i64::MAX);
        let record = HEAP.with(|h| h.borrow_mut().new_counted_record(&[child, 0]));
        HEAP.with(|h| {
            let h = h.borrow();
            assert_eq!(read_audited_scalar_field(&h, record, 0), Ok(2));
            assert!(read_audited_scalar_field(&h, record, 1).is_err());
            assert_eq!(read_audited_ref_field(&h, record, 1), Ok(child));
            assert_eq!(read_audited_ref_field(&h, record, 2), Ok(0));
            for i in [0, 3, -1] {
                assert!(read_audited_ref_field(&h, record, i).is_err());
            }
        });
        HEAP.with(|h| h.replace(previous));
    }

    #[test]
    fn checked_write_initializes_ref_and_rejects_layout_without_changing_edge() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = dartforge_arc_object_owned_v1(123, 40);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            HEAP.with(|h| {
                let mut heap = h.borrow_mut();
                assert!(write_checked_ref_field(&mut heap, object, 39, child, &[123, 40]).is_ok());
                for (receiver, ix, layouts) in [
                    (object, 39, vec![124, 40]), (object, 39, vec![123, 39]),
                    (object, -1, vec![123, 40]), (object, 40, vec![123, 40]),
                    (0, 39, vec![123, 40]), (child, 0, vec![123, 40]),
                    (object, 39, vec![123]),
                ] {
                    assert!(write_checked_ref_field(&mut heap, receiver, ix, 0, &layouts).is_err());
                    assert_eq!(read_audited_ref_field(&heap, object, 39), Ok(child));
                }
            });
            dartforge_arc_release(child);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(child), 1);
            HEAP.with(|h| assert!(write_checked_ref_field(&mut h.borrow_mut(), object, 39, 0, &[123, 40]).is_ok()));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(child), 0);
            dartforge_arc_release(object);
            HEAP.with(|h| h.replace(previous));
        }
    }

    #[test]
    fn read_borrows_edge_and_owned_copy_survives_replacement() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = dartforge_arc_object_owned_v1(123, 40);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_write_field_ref_v1(object, 0, 0);
            dartforge_arc_write_field_ref_v1(object, 1, smi::of(42).unwrap());
            dartforge_arc_write_field_ref_v1(object, 39, child);
            dartforge_arc_write_field_scalar_v1(object, 37, child);
            dartforge_arc_release(child);
            dartforge_arc_collect();
            let stats = HEAP.with(|h| h.borrow().stats());
            assert_eq!(dartforge_arc_read_field_ref_v1(object, 0), 0);
            assert_eq!(dartforge_arc_read_field_ref_v1(object, 1), smi::of(42).unwrap());
            assert_eq!(dartforge_arc_read_field_ref_v1(object, 39), child);
            assert_eq!(dartforge_arc_observe_heap_v1(child), 1);
            HEAP.with(|h| {
                let h = h.borrow();
                for (receiver, ix) in [(object, 37), (object, -1), (object, 40), (0, 0), (child, 0)] {
                    assert!(read_audited_ref_field(&h, receiver, ix).is_err());
                }
                // The never-written field (zeroed, marked scalar) is null.
                assert_eq!(read_audited_ref_field(&h, object, 38), Ok(0));
                assert_eq!(h.stats().allocations, stats.allocations);
                assert_eq!(h.stats().collections, stats.collections);
            });
            dartforge_arc_write_field_ref_v1(object, 39, 0);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(child), 0);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_write_field_ref_v1(object, 39, child);
            dartforge_arc_release(child);
            let copy = dartforge_arc_read_field_ref_v1(object, 39);
            assert_eq!(copy, child);
            dartforge_arc_retain(copy);
            dartforge_arc_write_field_ref_v1(object, 39, 0);
            dartforge_arc_release(object);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(object), 0);
            assert_eq!(dartforge_arc_observe_heap_v1(copy), 3);
            dartforge_arc_release(copy);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(copy), 0);
            HEAP.with(|h| { h.replace(previous); });
        }
    }
}

#[cfg(test)]
mod tests_late_ref_read {
    use super::*;

    #[test]
    fn late_ref_tells_empty_zero_written_null_and_scalar_bits() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = dartforge_arc_object_owned_v1(123, 40);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            let before = HEAP.with(|h| h.borrow().stats());
            assert_eq!(dartforge_arc_read_field_late_ref_v1(object, 39), 0);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.mark_late((object, 39));
                assert!(read_audited_late_ref_field(&h, object, 39).is_err());
                h.unmark_late((object, 39));
                for (bits, marked) in [(child, false), (42, false), (0, true), (child, true)] {
                    h.set_field(object, 38, bits, false);
                    if marked { h.mark_late((object, 38)); } else { h.unmark_late((object, 38)); }
                    assert!(read_audited_late_ref_field(&h, object, 38).is_err());
                }
                for (receiver, ix) in [(0, 0), (object, -1), (object, 40)] {
                    assert!(read_audited_late_ref_field(&h, receiver, ix).is_err());
                }
                assert_eq!(h.stats().allocations, before.allocations);
                assert_eq!(h.stats().collections, before.collections);
            });
            for value in [0, smi::of(42).unwrap(), child] {
                dartforge_arc_write_field_ref_v1(object, 39, value);
                HEAP.with(|h| h.borrow_mut().mark_late((object, 39)));
                assert_eq!(dartforge_arc_read_field_late_ref_v1(object, 39), value);
            }
            dartforge_arc_release(child);
            let copy = dartforge_arc_read_field_late_ref_v1(object, 39);
            dartforge_arc_retain(copy);
            dartforge_arc_write_field_ref_v1(object, 39, 0);
            dartforge_arc_release(object);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(copy), 3);
            assert_eq!(dartforge_arc_observe_heap_v1(object), 0);
            HEAP.with(|h| assert!(read_audited_late_ref_field(&h.borrow(), object, 39).is_err()));
            dartforge_arc_release(copy);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(copy), 0);
            HEAP.with(|h| { h.replace(previous); });
        }
    }
}

#[cfg(test)]
mod tests_field_write {
    use super::*;

    #[test]
    fn write_tells_bits_from_edges_and_replaces_without_consuming_tokens() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = dartforge_arc_object_owned_v1(123, 40);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            let before = HEAP.with(|h| h.borrow().stats());
            for ix in [0, 37, 38] {
                dartforge_arc_write_field_ref_v1(object, ix, child);
                dartforge_arc_write_field_ref_v1(object, ix, child);
            }
            for (ix, bits) in [(1, i64::MAX), (32, (-0.0_f64).to_bits() as i64), (39, 0x7ff80000deadbeef)] {
                dartforge_arc_write_field_scalar_v1(object, ix, bits);
                assert_eq!(dartforge_arc_read_field_scalar_v1(object, ix), bits);
            }
            HEAP.with(|h| {
                let h = h.borrow();
                assert_eq!(h.stats().allocations, before.allocations);
                assert_eq!(h.stats().collections, before.collections);
                assert_eq!(h.observe_code_owner(object), 3);
                assert_eq!(h.observe_code_owner(child), 3);
            });
            dartforge_arc_release(child);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(child), 1);
            // The same bits stop constituting an edge in both maps.
            for ix in [0, 37] { dartforge_arc_write_field_scalar_v1(object, ix, child); }
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(child), 1);
            dartforge_arc_write_field_ref_v1(object, 38, 0);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(child), 0);
            for ix in [0, 37] { assert_eq!(dartforge_arc_read_field_scalar_v1(object, ix), child); }
            dartforge_arc_write_field_ref_v1(object, 33, object);
            dartforge_arc_collect();
            dartforge_arc_write_field_scalar_v1(object, 33, 0);
            dartforge_arc_release(object);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(object), 0);
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn invalid_receiver_and_index_neither_write_nor_collect() {
        let mut heap = Heap::new(true);
        let object = heap.allocate_instance(123, 40);
        heap.retain_code_owner(object);
        let boxed = heap.box_int(i64::MAX);
        heap.retain_code_owner(boxed);
        let before = heap.object(object).unwrap().to_vec();
        let stats = heap.stats();
        for reference in [false, true] {
            for (receiver, ix) in [(object, -1), (object, 40), (0, 0), (boxed, 0)] {
                assert!(write_audited_field(&mut heap, receiver, ix, 0, reference).is_err());
                assert_eq!(heap.object(object).unwrap().to_vec(), before);
                assert_eq!(heap.stats().allocations, stats.allocations);
                assert_eq!(heap.stats().collections, stats.collections);
            }
        }
    }
}

#[cfg(test)]
mod tests_owned_instance {
    use super::*;

    #[test]
    fn owned_instance_registers_methods_before_allocating_and_preserves_token() {
        use std::sync::{OnceLock, atomic::{AtomicUsize, Ordering}};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        extern "C" fn input(_: i64, _: *const i64, _: *const i64) -> i64 { 85 }
        extern "C" fn table() -> *const i64 {
            static TABLE: OnceLock<[i64; 4]> = OnceLock::new();
            CALLS.fetch_add(1, Ordering::Relaxed);
            TABLE.get_or_init(|| [32001, 1, 42, input as *const () as usize as i64]).as_ptr()
        }
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        HEAP.with(|h| h.borrow_mut().enable_arc());
        for _ in 0..2 {
            // SAFETY: valid static table with one durable native entry.
            let object = unsafe { dartforge_arc_object_owned_t_v1(32001, 1, table) };
            assert_eq!(class_method(32001, 42), Some(input as *const () as usize));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(object), 3);
            assert_eq!(dartforge_arc_read_field_scalar_v1(object, 0), 0);
            dartforge_arc_release(object);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(object), 0);
        }
        assert_eq!(CALLS.load(Ordering::Relaxed), 1);
        HEAP.with(|h| { h.replace(previous); });
    }

    #[test]
    fn zeroed_instance_and_token_survive_stress_until_a_release() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            for fields in [0, 1, 32, 33, 40] {
                let object = dartforge_arc_object_owned_v1(123, fields);
                assert_eq!(dartforge_arc_observe_heap_v1(object), 3);
                // The next factory may collect before publishing its token.
                let other_one = dartforge_arc_object_owned_v1(124, 1);
                dartforge_arc_collect();
                HEAP.with(|h| {
                    let h = h.borrow();
                    let view = h.object(object).unwrap();
                    assert_eq!(view.class_id, 123);
                    assert_eq!(view.len(), fields as usize);
                    assert!(view.iter().all(|field| field == (0, false)));
                });
                let child = if fields > 0 {
                    let child = dartforge_arc_box_int_owned_v1(i64::MAX);
                    dartforge_object_set(object, fields - 1, child, 1);
                    dartforge_arc_release(child);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observe_heap_v1(child), 1);
                    assert_eq!(HEAP.with(|h| h.borrow().object(object).unwrap().field(fields as usize - 1)), (child, true));
                    Some(child)
                } else { None };
                dartforge_arc_release(other_one);
                dartforge_arc_release(object);
                assert_eq!(dartforge_arc_observe_heap_v1(object) & 2, 0);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observe_heap_v1(object), 0);
                assert_eq!(dartforge_arc_observe_heap_v1(other_one), 0);
                if let Some(child) = child { assert_eq!(dartforge_arc_observe_heap_v1(child), 0); }
            }
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    #[test]
    fn parameters_must_fit_in_header_without_truncation() {
        for class in [i32::MIN as i64, 0, i32::MAX as i64] {
            for fields in [0, 40, u16::MAX as i64] {
                assert_eq!(owned_instance_parameters(class, fields), Ok((class as i32, fields as usize)));
            }
        }
        for class in [i32::MIN as i64 - 1, i32::MAX as i64 + 1] {
            assert!(owned_instance_parameters(class, 1).is_err());
        }
        for fields in [-1, u16::MAX as i64 + 1, i64::MAX] {
            assert!(owned_instance_parameters(123, fields).is_err());
        }
    }
}

#[cfg(test)]
mod tests_scalar_field {
    use super::*;

    #[test]
    fn read_checks_map_without_mistaking_bits_for_handle_or_collecting() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let object = HEAP.with(|h| {
                let mut h = h.borrow_mut();
                let child = h.allocate_str("reference field");
                h.retain_code_owner(child);
                let mut fields = vec![(0, false); 40];
                fields[0] = (child, false);
                fields[1] = (child, true);
                fields[2] = ((-0.0_f64).to_bits() as i64, false);
                fields[3] = (0x7ff80000deadbeef, false);
                fields[32] = (i64::MAX, false);
                fields[37] = (child, true);
                let object = h.new_object(123, &fields);
                h.retain_code_owner(object);
                h.collect();
                object
            });
            let before = HEAP.with(|h| h.borrow().stats());
            for i in [0, 2, 3, 32, 39] {
                let expected = HEAP.with(|h| h.borrow().object(object).unwrap().field(i).0);
                assert_eq!(dartforge_arc_read_field_scalar_v1(object, i as i64), expected);
            }
            HEAP.with(|h| {
                let h = h.borrow();
                for i in [1, 37] {
                    assert_eq!(read_audited_scalar_field(&h, object, i), Err("field is a managed reference"));
                }
                for i in [-1, 40] { assert!(read_audited_scalar_field(&h, object, i).is_err()); }
                assert!(read_audited_scalar_field(&h, 0, 0).is_err());
                assert_eq!(h.stats().allocations, before.allocations);
                assert_eq!(h.stats().collections, before.collections);
            });
            HEAP.with(|h| { h.replace(previous); });
        }
    }
}
