// Native runtime: the canonical collection constant.
//
// Unified space (docs/NATIVO-ESPACO-UNIFICADO.md §4.5, item 3, and the cut of
// §4.2): the legacy without the SDK from source (`dartforge_list_*`, `_map_*`,
// `_set_*`, `_iteration_*`, `_generic_len`) is gone; only what the getter of
// `const [...]` calls remains.

/// The canonical collection constant (`lower/constantes.rs`, the getter of
/// `const [...]`): a list becomes a **new** `_ImmutableList` with the same
/// elements and the same `E` (the class does not change after publication,
/// docs/NATIVO-ESPACO-UNIFICADO.md §2.16); the caller uses the returned value.
/// An `_ImmutableList` and anything that is not a core list (`_ConstMap`/`_ConstSet`,
/// already unmodifiable by class) come back as they are.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_collection_mark_unmodifiable(handle: i64) -> i64 {
    let is_list = HEAP.with(|h| {
        let h_ = h.borrow();
        h_.is_list(handle) && h_.class(handle) != crate::layout::cid::IMMUTABLE_LIST
    });
    if !is_list {
        return handle;
    }
    // `handle` is an argument: the generated code keeps it rooted.
    let new = HEAP.with(|h| h.borrow_mut().unmodifiable_list_of(handle));
    type_copied_list(new, handle, crate::layout::cid::IMMUTABLE_LIST);
    new
}
