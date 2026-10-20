//! The type universe (RTI, `src/types.rs`) and the event loop (`src/events.rs`),
//! through the ABI that generated code uses.
//!
//! Each test runs on its own thread: the runtime state is per thread (one
//! isolate per thread).

use dartforge_runtime::abi::*;

/// Reads a recipe (the text becomes a heap `String`, as in generated code).
#[allow(unsafe_code)]
fn recipe(r: &str) -> i64 {
    // SAFETY: `r` is valid UTF-8 with `r.len()` bytes, alive during the call.
    let h = unsafe { dartforge_string_new(r.as_ptr(), r.len() as i64) };
    dartforge_rti_recipe(h)
}

fn sub(s: &str, t: &str) -> bool {
    dartforge_rti_subtype(recipe(s), recipe(t)) != 0
}

fn in_a_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[allow(unsafe_code)]
fn internal_recipe_ids_without_collecting_retaining_string_or_changing_pending() {
    in_a_thread(|| {
        dartforge_memory_arc_v1();
        for recipe in [
            "C350<C201>",
            "F<0;C201;1;C201;nome:C350!>",
            "R<C201;nome:C350>",
            "L<C201,C350>",
        ] {
            let mut previous = None;
            for pending in [false, true] {
                let mid = recipe.len() / 2;
                // SAFETY: ASCII recipes; both parts are valid regions.
                let (a, b) = unsafe {
                    (
                        dartforge_string_new(recipe.as_ptr(), mid as i64),
                        dartforge_string_new(recipe.as_ptr().add(mid), (recipe.len() - mid) as i64),
                    )
                };
                // The literals are canonical roots; the concatenation is dynamic.
                let text = dartforge_string_concat(a, b);
                dartforge_arc_retain(text);
                let trace = if pending {
                    let r = dartforge_stack_trace_get();
                    dartforge_throw_with_stack_trace(0, 3, r);
                    Some(r)
                } else {
                    None
                };
                let name = b"dartforge_rti_recipe";
                // SAFETY: constant name. Bit 0 forbids collection, bit 1 a new pending exception.
                unsafe {
                    dartforge_effects_before(name.as_ptr(), name.len() as i64, 3);
                }
                let ty = dartforge_rti_recipe(text);
                dartforge_effects_after();
                assert_eq!(dartforge_effects_level(), 0);
                assert_eq!(dartforge_exception_pending(), u8::from(pending));
                if let Some(trace) = trace {
                    assert_eq!(dartforge_exception_peek_ref(), 0);
                    assert_eq!(dartforge_stack_trace_get(), trace);
                    dartforge_exception_clear();
                }
                if let Some(id) = previous {
                    assert_eq!(ty, id);
                }
                previous = Some(ty);
                assert_eq!(dartforge_arc_observe_heap_v1(text), 3);
                dartforge_arc_release(text);
                dartforge_arc_collect();
                assert_eq!(
                    dartforge_arc_observe_heap_v1(text),
                    0,
                    "recipe {recipe}; pending {pending}"
                );
                // The canonical ID stays valid without retaining the recipe string.
                assert_eq!(dartforge_rti_subtype(ty, ty), 1);
            }
        }
    });
}

#[test]
#[allow(unsafe_code)]
fn set_rti_preserves_owner_pending_and_elements_on_decompact() {
    in_a_thread(|| {
        dartforge_memory_arc_v1();
        dartforge_rti_runtime_class(0, 201);
        dartforge_rti_runtime_class(4, 200);
        dartforge_rti_rule(8, recipe("C200<P0>"));
        let int_type = recipe("C200<C201>");
        let general_type = recipe("C200<D>");
        let value_list = [i64::MAX, i64::MIN];
        // SAFETY: word vector alive and complete for the compact list.
        let list = unsafe { dartforge_list_new(value_list.as_ptr(), 2, 8, 1) };
        dartforge_arc_retain(list);
        let define = |ty| {
            let name = b"dartforge_rti_set";
            // SAFETY: live constant name, with its exact length.
            unsafe {
                dartforge_effects_before(name.as_ptr(), name.len() as i64, 2);
            }
            dartforge_rti_set(list, ty);
            dartforge_effects_after();
            assert_eq!(dartforge_effects_level(), 0);
        };
        define(int_type);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(list), 3);
        define(general_type);
        assert_eq!(dartforge_exception_pending(), 0);
        define(int_type);
        // The call neither clears nor replaces a previous pending exception.
        let trace = dartforge_stack_trace_get();
        dartforge_throw_with_stack_trace(0, 3, trace);
        define(general_type);
        assert_eq!(dartforge_exception_pending(), 1);
        assert_eq!(dartforge_exception_peek_ref(), 0);
        assert_eq!(dartforge_stack_trace_get(), trace);
        dartforge_exception_clear();
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observe_heap_v1(list), 3);
        let first = dartforge_native_DartForge_list_get(list, 0);
        let second = dartforge_native_DartForge_list_get(list, 1);
        assert_eq!(dartforge_unbox_int(first), value_list[0]);
        assert_eq!(dartforge_unbox_int(second), value_list[1]);
        assert_ne!(dartforge_rti_e(list, general_type), 0);
        dartforge_arc_release(list);
        dartforge_arc_collect();
        for h in [list, first, second] {
            assert_eq!(dartforge_arc_observe_heap_v1(h), 0);
        }
        assert_eq!(dartforge_exception_pending(), 0);
    });
}

#[test]
fn late_field_tells_zero_value_from_missing_write() {
    in_a_thread(|| {
        let first = dartforge_object_new(1, 1);
        let second = dartforge_object_new(1, 1);
        assert_eq!(dartforge_late_field_initialized(first, 0), 0);
        dartforge_object_set(first, 0, 0, 0);
        dartforge_late_field_mark_initialized(first, 0);
        assert_eq!(dartforge_late_field_initialized(first, 0), 1);
        assert_eq!(dartforge_object_get(first, 0), 0);
        assert_eq!(dartforge_late_field_initialized(second, 0), 0);
    });
}

/// Subtyping (specification, "Subtypes"): classes with arguments through the
/// supertype rules, nullables, `FutureOr`, functions (contravariant
/// parameters), and the evaluation of a recipe with `P<i>` in the type of
/// `this`.
#[test]
fn subtyping_and_evaluation() {
    in_a_thread(|| {
        // 0 = Object; 201 and 202 classes without parameters; 210 = List<E>, 211 =
        // Iterable<E>, 220 = Future<T> (program classes: cid ≥ 128, outside the
        // runtime's fixed cids, docs/NATIVO-ESPACO-UNIFICADO.md §2.4).
        dartforge_rti_runtime_class(10, 0);
        dartforge_rti_runtime_class(9, 220);
        dartforge_rti_rule(210, recipe("C211<P0>"));
        dartforge_rti_rule(201, recipe("C202"));
        assert!(sub("C210<C201>", "C211<C201>"));
        assert!(
            sub("C210<C201>", "C211<C202>"),
            "covariant through rule 201 <: 202"
        );
        assert!(!sub("C210<C202>", "C211<C201>"));
        assert!(!sub("C211<C201>", "C210<C201>"));
        assert!(sub("C201", "C0"), "everything is Object");
        assert!(!sub("U", "C0"), "Null is not Object");
        assert!(sub("U", "C201?"));
        assert!(!sub("C201?", "C201"));
        assert!(sub("C201", "C201?"));
        assert!(sub("C220<C201>", "O<C201>"));
        assert!(sub("C201", "O<C201>"));
        assert!(!sub("C202", "O<C201>"));
        assert!(sub("N", "C201"), "Never is the bottom");
        assert!(sub("C201", "D"), "dynamic is the top");
        // R Function(P): covariant return, contravariant parameter.
        assert!(sub("F<0;C201;1;C0;>", "F<0;C0;1;C201;>"));
        assert!(!sub("F<0;C0;1;C201;>", "F<0;C201;1;C0;>"));
        assert!(!sub("F<0;D;1;D;>", "F<0;D;2;C0,C0;>"), "aridade");

        // `P0` evaluated in the type of `this` (List<C201>) seen as Iterable.
        let obj = dartforge_object_new(210, 0);
        dartforge_rti_set(obj, recipe("C210<C201>"));
        let t = dartforge_rti_eval(recipe("C211<P0>"), obj, 211, 0);
        assert_eq!(t, recipe("C211<C201>"));
        assert!(dartforge_rti_e(obj, recipe("C211<C202>")) != 0);
        assert!(dartforge_rti_e(obj, recipe("C211<U>")) == 0);
        // `M0` of the tuple.
        let t = dartforge_rti_eval(recipe("C210<M0>"), 0, 0, recipe("L<C202>"));
        assert_eq!(t, recipe("C210<C202>"));
    });
}

/// The SDK's list allocators receive `L<E>` through the ABI. The slice of a
/// `_List<E>` and the `_ImmutableList<E>` copy keep E in the metadata, even when
/// the concrete class of the result differs from the source.
#[test]
#[allow(unsafe_code)]
fn list_allocators_preserve_type_argument() {
    in_a_thread(|| {
        // 200 = List<E> (a program class, cid ≥ 128); the concrete ones have the
        // contract's fixed cids (docs/NATIVO-ESPACO-UNIFICADO.md §2.4):
        // 8 = _List<E>, 9 = _ImmutableList<E>, 10 = _GrowableList<E>.
        dartforge_rti_runtime_class(4, 200);
        for class in [8, 9, 10] {
            dartforge_rti_rule(class, recipe("C200<P0>"));
        }

        let fixed_one = dartforge_native_List_allocate(dartforge_box_int(2), recipe("L<C201>"));
        assert!(dartforge_rti_e(fixed_one, recipe("C200<C201>")) != 0);
        let piece = dartforge_native_List_slice(fixed_one, 0, 2, 1);
        assert!(dartforge_rti_e(piece, recipe("C200<C201>")) != 0);

        let unmodifiable = dartforge_native_ImmutableList_from(fixed_one, 0, 2, recipe("L<C201>"));
        assert!(dartforge_rti_e(unmodifiable, recipe("C200<C201>")) != 0);
        assert!(dartforge_rti_e(unmodifiable, recipe("C200<C202>")) == 0);

        let unmodifiable_from_copy = dartforge_native_Internal_makeFixedListUnmodifiable(fixed_one);
        assert!(dartforge_rti_e(unmodifiable_from_copy, recipe("C200<C201>")) != 0);
        let fixed_from_copy = dartforge_native_Internal_makeListFixedLength(unmodifiable_from_copy);
        assert!(dartforge_rti_e(fixed_from_copy, recipe("C200<C201>")) != 0);

        let mutable = dartforge_native_GrowableList_allocate(fixed_one, recipe("L<C201>"));
        assert!(dartforge_rti_e(mutable, recipe("C200<C201>")) != 0);
    });
}

thread_local! {
    static ORDER: std::cell::RefCell<Vec<i64>> = const { std::cell::RefCell::new(Vec::new()) };
}

extern "C" fn call(closure: i64) -> i64 {
    let code = dartforge_closure_code(closure);
    ORDER.with(|o| o.borrow_mut().push(code));
    0
}

/// Microtasks before any timer; timers by (deadline, sequence); a cancelled
/// timer does not run.
#[test]
fn event_loop_in_vm_order() {
    in_a_thread(|| {
        let c = |k: i64| dartforge_tearoff(k);
        dartforge_native_DartForge_Timer_new(20, c(1), 0);
        dartforge_native_DartForge_Timer_new(0, c(2), 0);
        let cancelled = dartforge_native_DartForge_Timer_new(0, c(3), 0);
        dartforge_native_DartForge_Timer_new(0, c(5), 0);
        dartforge_native_DartForge_scheduleImmediate(c(4));
        dartforge_native_DartForge_Timer_cancel(cancelled);
        dartforge_event_loop(call);
        let order = ORDER.with(|o| o.borrow().clone());
        assert_eq!(order, vec![4, 2, 5, 1]);
    });
}
