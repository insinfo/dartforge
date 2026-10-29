//! Table of the runtime externs: the LLVM declaration and the EFFECTS of each one.
//!
//! Single source of truth (docs/NATIVO-PLANO.md §6.5, "effects table"):
//! the emitter declares the externs from here, and step 6 of the plan will
//! use the effects to (a) not require live roots nor a frame around
//! an extern that does not allocate — the equivalent of the VM's LEAF entries
//! (`runtime_entry.cc:778`) and of Dartino's `gc-leaf-function` — and (b)
//! skip the pending exception check after an extern that does not
//! throw ("does not allocate ⇒ does not throw": throwing allocates the error and the stack trace).
//!
//! At this stage all of them are marked conservatively (allocates, throws):
//! a wrong optimistic mark is exactly the defect of wrong stack maps,
//! only on our side — it can only be changed together with a test that
//! runs the extern under `DARTFORGE_GC_STRESS=1` and requires zero collections.

/// What a call to the extern may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effects {
    /// May allocate on the heap (hence, collect): it is a collection point.
    pub allocates: bool,
    /// May leave a pending exception.
    pub throws: bool,
    /// May call generated Dart code (which, in turn, allocates and throws).
    pub calls_dart: bool,
}

/// Allocates (a collection point), but neither throws nor calls Dart code: the
/// simple allocations and reads of the runtime (checked in the code of each
/// one). `optimize/effects.rs` relies on this to know who does not throw.
pub const ALLOCATES_WITHOUT_THROWING: Effects = Effects {
    allocates: true,
    throws: false,
    calls_dart: false,
};

/// Conservative mark: allocates and throws.
pub const CONSERVATIVE: Effects = Effects {
    allocates: true,
    throws: true,
    calls_dart: false,
};

/// A runtime extern: the LLVM declaration and the effects.
pub struct Extern {
    pub decl: &'static str,
    pub effects: Effects,
}

impl Extern {
    /// The name of the symbol (`dartforge_…`), between the `@` and the `(`.
    pub fn name(&self) -> &'static str {
        let ini = self.decl.find('@').map_or(0, |i| i + 1);
        let end = self.decl[ini..]
            .find('(')
            .map_or(self.decl.len(), |i| ini + i);
        &self.decl[ini..end]
    }
}

/// All the externs the generated code may call.
pub const EXTERNS: &[Extern] = &[
    Extern {
        decl: "declare i64 @dartforge_arc_closure_owned_v1(i64, i64, i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_string_new(ptr, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_string_concat(i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i8 @dartforge_equal(i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_register_class_name(i64, ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_canonical_tearoff(i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_register_instantiation(ptr)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_args_set_tuple(ptr, ptr, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_register_tearoff(ptr, ptr, i64, i8)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_object_new(i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_object_get(i64, i64)",
        // Reads/writes a field: does not allocate (the emitter expands it inline with a
        // constant index).
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // `_StackTrace.toString` (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
        // §13.14): the stored text, or the addresses of the `throw`
        // symbolized now (allocates the text).
        decl: "declare i64 @dartforge_stack_trace_string(i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // The registration of the stack trace table of the image (§13.14): neither allocates nor
        // throws.
        decl: "declare void @dartforge_register_stack_trace(ptr, ptr)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // The position of a `dart:async` field that the stack trace walks
        // (§13.14): neither allocates on the collector heap nor throws.
        decl: "declare void @dartforge_register_stack_trace_field(i64, ptr, i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // The tear-off entries that the stream branches of the stack trace
        // recognize (§13.14). Neither allocates on the collector heap nor throws.
        decl: "declare void @dartforge_register_stack_trace_tearoff(ptr, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // The stack trace stack (§13.14): the frame of an `async` body or the
        // listener of `handleValue`, pushed at the entry; returns the
        // depth. Neither allocates on the collector heap nor throws.
        decl: "declare i64 @dartforge_stack_trace_enter(i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // The exit: cuts the stack trace stack at the depth of the entry.
        decl: "declare void @dartforge_stack_trace_exit(i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // The write barrier (`llvm/mod.rs`, `emit_barrier`): remembers the
        // old object; neither allocates nor throws.
        decl: "declare void @dartforge_remember(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // ARC memory (docs/ARC-IMPLEMENTACAO.md): turns ARC on at the entry
        // (a full tracing collection and the registration of the live objects).
        decl: "declare void @dartforge_memory_arc_v1()",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // The same, with the nursery: the ARC of a program without the
        // published preparation (young objects uncounted until promotion).
        decl: "declare void @dartforge_memory_arc_nursery_v1()",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // The counted write of a `Ref` into a word of a `REFS` body (the
        // list element under ARC): neither allocates nor throws.
        decl: "declare void @dartforge_arc_write_ref(i64, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // Pure ARC: the references written inline into a freshly allocated
        // object start counting. Neither allocates nor throws.
        decl: "declare void @dartforge_arc_initial(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_global_receive_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_box_double_owned_v1(double) nounwind",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_read_field_scalar_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_register_return_owned_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_register_uniform_owned_v1(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_register_typed_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_take_typed_v1(i64, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_typed_target_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_invoke_uniform_v1(i64, i64, i64, i64) nounwind",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_invoke_uniform_closure_v1(i64, i64, i64) nounwind",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_take_return_owned_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_native_scope_open_v1() nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_native_scope_close_v1(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_env_ref_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_scalar_env_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_closure_context_v1(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_socket_recvfrom_owned_v1(i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_read_field_ref_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_read_field_late_ref_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_list_storage_ref_v1(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_list_shape_v1(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_list_capacity_v1(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_list_write_len_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_list_write_bits_v1(i64, i64, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_list_write_ref_v1(i64, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_list_write_data_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_list_len_v1(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_list_read_bits_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_list_ref_owned_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_list_ref_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_argument_owned_v1(ptr, ptr, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_box_int_owned_v1(i64) nounwind",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_box_simd_owned_v1(i64, i64, i64) nounwind",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_object_owned_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_object_owned_t_v1(i64, i64, ptr) nounwind",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_write_field_scalar_v1(i64, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_write_field_scalar_checked_v1(i64, i64, i64, ptr, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_read_field_scalar_checked_v1(i64, i64, ptr, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_write_field_ref_checked_v1(i64, i64, i64, ptr, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_write_field_ref_v1(i64, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_exception_owned_v1() nounwind",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_stack_trace_owned_v1() nounwind",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_throw_with_stack_trace_ref_v1(i64, i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_throw_ref_v1(i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i8 @dartforge_arc_observe_heap_v1(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_frame_open_v1(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_frame_load_v1(i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_frame_receive_v1(i64, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_frame_copy_v1(i64, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_frame_move_v1(i64, i64, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_frame_close_v1(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // The real ARC: the pure ARC with prompt death (prepared, exact tokens).
        decl: "declare void @dartforge_memory_arc_prompt_v1()",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // The real ARC's write that hands the value's token to the field.
        decl: "declare void @dartforge_arc_write_field_ref_move_v1(i64, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_virtual_tokens_v1() nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_retain(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_release(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_collect() nounwind",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_arc_register_drops_v1(ptr, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i8 @dartforge_arc_verify_abi(i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_object_set(i64, i64, i64, i8)",
        // Reads/writes a field: does not allocate (the emitter expands it inline with a
        // constant index).
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i8 @dartforge_late_field_initialized(i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_late_field_mark_initialized(i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_tearoff(i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_tearoff_typed(i64, i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare void @dartforge_gc_global_root(i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare void @dartforge_mark_constant(i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i8 @dartforge_exception_catchable()",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_register_isolates(ptr, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_register_type(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_register_trampoline(ptr, i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_closure_address(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_pointer_address(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_return_pointer(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        // The same entry under ARC, with a code owner of the result.
        decl: "declare i64 @dartforge_arc_ffi_return_pointer_owned_v1(i64, i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_typed_new(i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_typed_new_t(i64, i64, i64, ptr)",
        effects: CONSERVATIVE,
    },
    // The fast path of typed lists (`lower/typed_items.rs`): pure
    // functions of the handle. The length, the address of the elements, the type and the
    // immutability of a typed list do not change while it lives, and a
    // handle is only reused after the object dies — when the code no longer
    // uses it. So `memory(none)`: LLVM hoists them out of loops despite the
    // calls of the root registration. `speculatable`: on any handle
    // (even one that is not a typed list) they answer 0, without an effect.
    // The thread context (`runtime/src/heap.rs`, `Context`): the same
    // address while the thread lives — pure for the code of an activation.
    Extern {
        decl: "declare ptr @dartforge_context() memory(none) nounwind willreturn speculatable",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_typed_len(i64, i64, i64) memory(none) nounwind willreturn speculatable",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    // `fillRange` of a typed list (`lower/typed_items.rs`, `fill_typed`):
    // 1 if it filled, 0 for the SDK path.
    Extern {
        decl: "declare i64 @dartforge_typed_fill_int(i64, i64, i64, i64, i64) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_typed_fill_double(i64, i64, i64, i64, double) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    // Interpolation with unboxed `int` parts: (kind, bits) pairs.
    Extern {
        decl: "declare i64 @dartforge_string_join_typed(ptr, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // The same interpolation under ARC: roots the parts and hands over an owner.
        decl: "declare i64 @dartforge_arc_string_join_owned_v1(ptr, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    // The ABI of the caller checked against that of the closure (`closures.rs`).
    Extern {
        decl: "declare i64 @dartforge_closure_typed(i64, i64) memory(inaccessiblemem: read) nounwind willreturn",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    // The fields of inline objects (`GetField`/`SetField`): the address
    // comes from the field vector in the runtime heap, which only changes size through
    // calls without attributes; the fields are memory the module accesses.
    Extern {
        decl: "declare i64 @dartforge_object_fields(i64) memory(inaccessiblemem: read) nounwind willreturn speculatable",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_view_new(i64, i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_view_new_t(i64, i64, i64, i64, i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_unbox_int(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare double @dartforge_unbox_double(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i8 @dartforge_unbox_bool(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_exception_peek_ref()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_exception_throw(i64, i8)",
        effects: CONSERVATIVE,
    },
    Extern {
        // The interrupt request of the isolate (J01): only reads a byte.
        decl: "declare i8 @dartforge_pending_interrupt()",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // The loop safepoint: serves the control; an immediate `kill`
        // leaves the exception pending (not catchable).
        decl: "declare void @dartforge_safepoint()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i8 @dartforge_exception_pending()",
        // Only reads or clears the pending state: does not allocate.
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_exception_clear()",
        // Only reads or clears the pending state: does not allocate.
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_register_subclass(i64, i64)",
        effects: CONSERVATIVE,
    },
    // The `v is C` test by class: only reads the class graph (and writes its
    // caches, `core.rs`); neither allocates on the heap nor throws. Without the conservative
    // effects, the live references do not go to the roots frame on
    // every test, and LLVM merges equal tests.
    Extern {
        decl: "declare i8 @dartforge_is_subclass(i64, i64) memory(read) nounwind willreturn",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_stack_trace_get()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_throw_with_stack_trace(i64, i8, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_value_class(i64)",
        // The class of the value: only reads (docs/NATIVO-ESPACO-UNIFICADO.md §3.5).
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_i64(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_f64(double)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_bool(i8)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_to_string_handle(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_record_new(ptr, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_collection_mark_unmodifiable(i64)",
        // Returns a new `_ImmutableList` (§2.16): allocates, does not throw.
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_exception_new(i64, i8)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_state_error_new(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_argument_error_new(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_argument_error_value(i64, i8, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_argument_error_not_null(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_range_error_range(i64, i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_unsupported_error_new(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_unimplemented_error_new(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_assertion_error_new(i64, i8)",
        effects: CONSERVATIVE,
    },
    // The error constructors under ARC (`excecoes.rs`): the new object with a
    // code owner; they run Dart in a native scope.
    Extern {
        decl: "declare i64 @dartforge_arc_exception_new_owned_v1(i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_assertion_error_new_owned_v1(i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_state_error_new_owned_v1(i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_unsupported_error_new_owned_v1(i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_unimplemented_error_new_owned_v1(i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_argument_error_new_owned_v1(i64, i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_argument_error_value_owned_v1(i64, i64, i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_argument_error_not_null_owned_v1(i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_range_error_range_owned_v1(i64, i64, i64, i64, i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_type_error_new_owned_v1()",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_null_check_error_new_owned_v1()",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_late_error_new_owned_v1(i64, i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_stack_overflow_error_new_owned_v1()",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_no_such_method_error_new_owned_v1(i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_type_error_new()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_null_check_error_new()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_late_error_new(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_stack_overflow_error_new()",
        effects: CONSERVATIVE,
    },
    // The prologue found the stack at the limit (`Context::limite_da_pilha`).
    Extern {
        decl: "declare void @dartforge_stack_overflow()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i8 @dartforge_late_field_initializing(i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_late_field_set_initializing(i64, i64, i8)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_no_such_method_error_new(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_closure_entry(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i8 @dartforge_args_match(ptr, ptr)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arg_index(ptr, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_nsm_call()",
        effects: CONSERVATIVE,
    },
    // --- P2 (α): operators over dynamic/num (stopgap until P5) ---
    Extern {
        decl: "declare i64 @dartforge_dyn_op(i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    // --- P3 (α): iteration of a list or set ---
    Extern {
        decl: "declare i64 @dartforge_iteravel_get_ref(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_iteravel_get_bits(i64, i64)",
        effects: CONSERVATIVE,
    },
    // --- P5c (δ): source SDK — selectors, method tables, rejections ---
    Extern {
        decl: "declare ptr @dartforge_selector(ptr, i64, i64, ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_register_methods(i64, ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_member_rejected(i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_register_cids(ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_register_helper(ptr, i64, ptr)",
        effects: CONSERVATIVE,
    },
    // The JavaScript interop of WebAssembly modules (crate::js_interop): a
    // `String` argument of a snippet becomes a JavaScript string (no heap
    // allocation), a `String` result a Dart string; the export the adapter calls
    // when a snippet throws runs `_dartforgeJavaScriptError`.
    Extern {
        decl: "declare i64 @dartforge_js_string_to_js(i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_js_string_to_dart(i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare void @dartforge_js_throw()",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare void @dartforge_register_argument_name(ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_forward_nsm(i64, i64, i64, i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_register_sdk_version(ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_prepare_embedder()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_main_arguments()",
        effects: CONSERVATIVE,
    },
    // The area is native memory (a `Vec`), never the collector's heap.
    Extern {
        decl: "declare ptr @dartforge_globals_area(ptr)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    // As `dartforge_globals_area`: the table of areas is native memory.
    Extern {
        decl: "declare ptr @dartforge_globals_area_id(ptr, ptr)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i32 @dartforge_init(ptr, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_call_trace_entry(ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_call_trace_exit()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_object_new_t(i64, i64, ptr)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // The allocation with the static reference map (`emit_inline_allocation`).
        decl: "declare i64 @dartforge_object_new_m(i64, i64, i64)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_object_new_mt(i64, i64, i64, ptr)",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare void @dartforge_register_table(i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_register_tables(ptr, i64)",
        effects: CONSERVATIVE,
    },
    // --- P6: event loop and the overlay natives (`events.rs`) ---
    Extern {
        // The only point where the runtime calls Dart code (G8).
        decl: "declare void @dartforge_event_loop(ptr)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare void @dartforge_native_DartForge_scheduleImmediate(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_native_DartForge_Timer_new(i64, i64, i8)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_native_DartForge_Timer_cancel(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_native_DartForge_Timer_tick(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i8 @dartforge_native_DartForge_native_pump_supported()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_native_DartForge_native_pump_enable()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_native_DartForge_native_pump_disable()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i8 @dartforge_native_DartForge_native_pump_dispatcher_supported()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_native_DartForge_native_pump_set_dispatcher(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_native_DartForge_native_pump_clear_dispatcher()",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_native_Error_throwWithStackTrace(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_native_Error_trySetStackTrace(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        // Pure leaf (RTI universe, no heap): only aborts with a malformed table.
        decl: "declare void @dartforge_rti_init_table(ptr, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_list_from_table(ptr, i64)",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_list_from_table_g(ptr, i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_register_compound(i64, i64, i64, i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_compound(i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_pointer_new(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_compound_address(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_compound_new(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_handles_open()",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_ffi_handles_close(i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_handle_new(i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_handle_object(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_handles_close_with_object(i64, i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        // The same reads under ARC, with a code owner of the object.
        decl: "declare i64 @dartforge_arc_ffi_handles_close_with_object_owned_v1(i64, i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_arc_ffi_handle_object_owned_v1(i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_compound_copy(i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_copy_compound(ptr, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_register_callback(ptr, i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_enter(ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_late_return(ptr) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_late_calls() nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i8 @dartforge_ffi_callback_exit(ptr, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_ffi_callback_post(ptr, ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_sync(ptr, ptr, i64)",
        // Waits for the owner; this foreign thread never enters Dart or the
        // managed heap. Effects describe execution on the calling thread.
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_closure(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_callback_pointer(i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        // The same entries under ARC, with a code owner of the result.
        decl: "declare i64 @dartforge_arc_ffi_callback_closure_owned_v1(i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_arc_ffi_callback_pointer_owned_v1(i64, i64, i64)",
        effects: Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_ffi_native_symbol(ptr, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_typed_external(i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_typed_external_t(i64, i64, i64, i64, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_recipe(i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_rti_eval(i64, i64, i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    // `dartforge_rti_eval` plus the use site's cache and the thread's memo.
    Extern {
        decl: "declare i64 @dartforge_rti_eval_cache(ptr, i64, i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_rti_tuple_join(i64, i64, i64, i64)",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_rti_set(i64, i64)",
        // Interns IDs and may reformat the list with boxes/GC, without throwing a
        // Dart exception nor running Dart getters/callbacks.
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_list_elem_exact(i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_list_elem_exact_cache(ptr, i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_rti_set_cache(ptr, i64, i64)",
        // Interns IDs and may reformat the list with boxes/GC, without throwing a
        // Dart exception nor running Dart getters/callbacks.
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_rti_named_record(i64, i64, i64)",
        effects: Effects {
            allocates: true,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i8 @dartforge_rti_e(i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i8 @dartforge_rti_e_cache(ptr, i64, i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare void @dartforge_rti_as_with_context(i64, i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_tuple_len(i64)",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        decl: "declare i64 @dartforge_nsm_mismatch(i64, i64, i64, ptr, ptr)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare void @dartforge_rti_bound(i64, i64, i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_string(i64)",
        effects: CONSERVATIVE,
    },
    Extern {
        // The same text under ARC, with a code owner of the result.
        decl: "declare i64 @dartforge_arc_rti_string_owned_v1(i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_arc_rti_object_type_owned_v1(i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_rti_object_type(i64)",
        effects: CONSERVATIVE,
    },
    // ─── Unified space (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ──────────────
    Extern {
        // (cid, words, flags): the slow allocation of `@df.allocate` (P0).
        decl: "declare i64 @dartforge_allocate(i64, i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // (len, two): the slow path of `@df.string_allocate` (P1).
        decl: "declare i64 @dartforge_string_allocate(i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // `@df.string_allocate` under ARC: the new string with a code owner.
        decl: "declare i64 @dartforge_arc_string_allocate_owned_v1(i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // `TearOff` under ARC: the canonical tear-off with a code owner.
        decl: "declare i64 @dartforge_arc_tearoff_owned_v1(i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // The string of a constant becomes the canonical literal (`constants.rs`).
        decl: "declare i64 @dartforge_canonical_constant(i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // Computes and writes the hash into the header of the string (P1).
        decl: "declare i64 @dartforge_string_hash(i64) nounwind willreturn",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // The same code units? The slow path of `@df.string_equals` (P1).
        decl: "declare i8 @dartforge_string_equals(i64, i64) memory(read) nounwind willreturn",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // (words, n, cid, shape): the literal list (P3).
        decl: "declare i64 @dartforge_list_new(ptr, i64, i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        decl: "declare i64 @dartforge_arc_list_new_owned_v1(ptr, i64, i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // The literal record under ARC (`AllocRecord`), with a code owner.
        decl: "declare i64 @dartforge_arc_record_from_refs_owned_v1(ptr, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // (list, Ref): the `add` that grows the storage (P3).
        decl: "declare i64 @dartforge_list_append(i64, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
    Extern {
        // (start, end): the section of the static objects of an image (P0).
        decl: "declare void @dartforge_register_image(ptr, ptr) nounwind",
        effects: Effects {
            allocates: false,
            throws: false,
            calls_dart: false,
        },
    },
    Extern {
        // (Refs, n): the positional record (P2).
        decl: "declare i64 @dartforge_record_from_refs(ptr, i64) nounwind",
        effects: ALLOCATES_WITHOUT_THROWING,
    },
];

/// The runtime data that the generated code references (not functions),
/// `(symbol, declaration)`: none. The static `bool` boxes are read from the
/// thread `Context` (`layout::context::VERDADEIRO`/`FALSO`), because between the
/// executable and the SDK DLL each image has its own copy of the runtime data
/// (docs/NATIVO-ESPACO-UNIFICADO.md §4.10, item 9).
pub const GLOBALS: &[(&str, &str)] = &[];

/// Effects of extern `name`; an unknown one is conservative.
pub fn effects_of(name: &str) -> Effects {
    // The generic Owned thunk has the effects of the base, plus the retain.
    if let Some(base) = name.strip_prefix(crate::optimize::arc::GENERIC_OWNED_PREFIX) {
        return Effects {
            allocates: true,
            ..effects_of(base)
        };
    }
    // The scope thunk has the effects of the base (the scope does not allocate).
    if let Some(base) = name.strip_prefix(crate::optimize::arc::NATIVE_SCOPE_PREFIX) {
        return effects_of(base);
    }
    // A JavaScript snippet may call back into Dart (`Function.toJS`).
    if name.starts_with(crate::js_interop::SNIPPET_PREFIX) {
        return Effects {
            allocates: true,
            throws: true,
            calls_dart: true,
        };
    }
    // The `@df.*` helpers of each unified-space package called through
    // `CallRuntime` (docs/NATIVO-ESPACO-UNIFICADO.md §3.5).
    for table in [
        super::ir_strings::HELPER_EFFECTS,
        super::ir_boxes::HELPER_EFFECTS,
        super::ir_lists::HELPER_EFFECTS,
        super::ir_typed::HELPER_EFFECTS,
    ] {
        if let Some(&(_, allocates, throws)) = table.iter().find(|(n, _, _)| *n == name) {
            return Effects {
                allocates,
                throws,
                calls_dart: false,
            };
        }
    }
    static TABLE: std::sync::OnceLock<std::collections::HashMap<&'static str, Effects>> =
        std::sync::OnceLock::new();
    // The source is the runtime table (`crates/runtime/effects.tsv`,
    // docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.8), with one line per
    // extern; the marks of [`EXTERNS`] are the same information next to the
    // declaration, and the `runtime_table_agrees_with_declarations` test
    // keeps them equal.
    TABLE
        .get_or_init(|| {
            dartforge_runtime::effects::EFFECTS
                .iter()
                .map(|&(n, allocates, throws, calls_dart)| {
                    (
                        n,
                        Effects {
                            allocates,
                            throws,
                            calls_dart,
                        },
                    )
                })
                .collect()
        })
        .get(name)
        .copied()
        .unwrap_or(CONSERVATIVE)
}

/// The compilation checks the effects table (`DARTFORGE_EFFECTS=check`):
/// each call to an extern marked as not collecting or not throwing goes between
/// `dartforge_effects_before` and `dartforge_effects_after`, and the runtime
/// terminates the process, naming it, if the extern collects or leaves a
/// pending exception (`runtime/src/efeitos_conferir.rs`). Only for the tests:
/// that is how a mark goes from 1 to 0.
pub fn check_effects() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("DARTFORGE_EFFECTS").is_ok_and(|v| v == "check"))
}

/// The marks that the check of `name` carries to the runtime: bit 0, does not collect;
/// bit 1, does not throw. 0 = nothing to check (or it is not an extern of the table).
pub fn marks_to_check(name: &str) -> i64 {
    if !dartforge_runtime::effects::EFFECTS
        .iter()
        .any(|(n, ..)| *n == name)
        || name == "dartforge_effects_before"
        || name == "dartforge_effects_after"
        || name == "dartforge_effects_level"
        || name == "dartforge_effects_restore"
    {
        return 0;
    }
    let e = effects_of(name);
    // The sabotaged leaf (`leaf:<name>`, D7 of §7.3) is checked as a
    // leaf: a collection inside it terminates the process, naming it.
    let allocates = e.allocates && !crate::target::sabotaged_leaf(name);
    i64::from(!allocates && !e.calls_dart) | (i64::from(!e.throws && !e.calls_dart) << 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_well_formed_names() {
        let mut names: Vec<&str> = EXTERNS.iter().map(Extern::name).collect();
        assert!(
            names.iter().all(|n| n.starts_with("dartforge_")),
            "{names:?}"
        );
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "extern declarada duas vezes");
    }

    #[test]
    fn runtime_table_agrees_with_declarations() {
        for e in EXTERNS {
            let t = effects_of(e.name());
            assert_eq!(
                (t.allocates, t.throws),
                (e.effects.allocates, e.effects.throws),
                "{}: the mark next to the declaration differs from that of crates/runtime/effects.tsv",
                e.name()
            );
            assert!(
                !e.effects.calls_dart || t.calls_dart,
                "{}: runs Dart in the declaration and not in the table",
                e.name()
            );
        }
    }

    #[test]
    fn timer_tick_and_native_pump_have_scalar_ownership_contracts() {
        use crate::hir::{Instruction, Type};
        use crate::optimize::arc::{Ownership, contract, runtime_call_contract};
        use dartforge_runtime::ownership::{ParamMode, ResultMode};

        for (name, result, ty) in [
            (
                "dartforge_native_DartForge_Timer_tick",
                ResultMode::ScalarI64,
                Type::I64,
            ),
            (
                "dartforge_native_DartForge_native_pump_supported",
                ResultMode::ScalarI8,
                Type::I8,
            ),
            (
                "dartforge_native_DartForge_native_pump_enable",
                ResultMode::ScalarI64,
                Type::I64,
            ),
            (
                "dartforge_native_DartForge_native_pump_disable",
                ResultMode::ScalarI64,
                Type::I64,
            ),
        ] {
            let c = contract(name).unwrap();
            assert_eq!(c.result, result);
            let timer = name.ends_with("Timer_tick");
            assert_eq!(
                c.parameters,
                if timer {
                    &[ParamMode::Scalar][..]
                } else {
                    &[][..]
                }
            );
            assert!(!c.persistent_retention && !c.calls_dart);
            assert!(c.invalidates_borrows && c.may_fail);
            let args = if timer {
                vec![(
                    crate::hir::Operand::Constant(crate::hir::Constant::Int(42)),
                    Type::I64,
                )]
            } else {
                vec![]
            };
            let call = Instruction::CallRuntime {
                name: name.into(),
                args: args.clone(),
                ret_ty: ty,
            };
            assert_eq!(
                runtime_call_contract(&call).unwrap().result,
                Ownership::Trivial
            );
            let incorrect = Instruction::CallRuntime {
                name: name.into(),
                args,
                ret_ty: Type::Ref,
            };
            assert!(runtime_call_contract(&incorrect).is_err());
            let declaration = EXTERNS.iter().find(|e| e.name() == name).unwrap();
            assert_eq!(declaration.effects, CONSERVATIVE);
        }
    }

    #[test]
    fn conservative_effects_by_default() {
        assert_eq!(effects_of("dartforge_list_push"), CONSERVATIVE);
        assert_eq!(effects_of("nao_existe"), CONSERVATIVE);
    }
}
