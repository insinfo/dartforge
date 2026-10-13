// Checking of the extern effects table (`crates/runtime/efeitos.tsv`,
// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.8).
//
// With `DARTFORGE_EFEITOS=conferir` at compile time, generated code calls
// `dartforge_efeitos_antes(name, marks)` before and `dartforge_efeitos_depois()`
// after every extern that the table marks `coleta = 0` (bit 0 of the marks) or
// `lanca = 0` (bit 1). Here the mark is put to the test:
//
// * `coleta = 0`: a collection while the extern runs terminates the process
//   (`heap::forbid_collection`), naming it;
// * `lanca = 0`: if the extern entered without a pending exception and left with
//   one, the process is terminated, naming it.
//
// A wrong optimistic mark is the emitter's most dangerous defect (a missing root,
// an exception nobody checks): a mark only goes from 1 to 0 after the corpus has
// run this way, preferably with `--gc-stress`. Without the variable at compile
// time, nothing here is called.

thread_local! {
    /// The checked externs running on this thread: (name, marks, the pending
    /// exception at entry).
    static EXTERNS_UNDER_CHECK: std::cell::RefCell<Vec<(String, i64, u8)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The entry of a checked extern.
///
/// # Safety
/// `name` points to `len` bytes (the extern's name, a module constant).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_efeitos_antes(name: *const u8, len: i64, marks: i64) {
    // SAFETY: guaranteed by the caller.
    let bytes = unsafe { std::slice::from_raw_parts(name, usize::try_from(len).unwrap_or(0)) };
    let name = String::from_utf8_lossy(bytes).into_owned();
    let pending = dartforge_exception_pending();
    if marks & 1 != 0 {
        crate::heap::forbid_collection(name.clone());
    }
    EXTERNS_UNDER_CHECK.with(|p| p.borrow_mut().push((name, marks, pending)));
}

/// The exit of the most recent checked extern.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_efeitos_depois() {
    let Some((name, marks, pending_before)) = EXTERNS_UNDER_CHECK.with(|p| p.borrow_mut().pop()) else {
        return;
    };
    if marks & 1 != 0 {
        crate::heap::allow_collection();
    }
    if marks & 2 != 0 && pending_before == 0 && dartforge_exception_pending() != 0 {
        eprintln!("dartforge: the extern {name} is marked lanca = 0 in efeitos.tsv and left a pending exception");
        std::process::abort();
    }
}

/// Returns the depth of the externs being checked on this thread.
///
/// ```
/// let level = dartforge_runtime::abi::dartforge_efeitos_nivel();
/// assert!(level >= 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_efeitos_nivel() -> i64 {
    EXTERNS_UNDER_CHECK.with(|p| p.borrow().len() as i64)
}

/// Abandons checks of externs crossed by a native unwind.
///
/// Removes only the levels above the snapshot taken at function entry, releasing
/// their collection bans. Does not check a normal exit nor change the pending
/// exception. Neither collects nor runs Dart; lowering also restores the roots at
/// the landing pad.
///
/// # Panics
/// Negative level or above the current depth; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let level = dartforge_efeitos_nivel();
/// dartforge_efeitos_restaurar(level);
/// assert_eq!(dartforge_efeitos_nivel(), level);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_efeitos_restaurar(level: i64) {
    let level = usize::try_from(level).expect("negative effects-check level");
    EXTERNS_UNDER_CHECK.with(|p| {
        let mut stack = p.borrow_mut();
        assert!(level <= stack.len(), "stale effects-check level");
        while stack.len() > level {
            let (_, marks, _) = stack.pop().unwrap();
            if marks & 1 != 0 { crate::heap::allow_collection(); }
        }
    });
}

#[cfg(test)]
mod tests_effects_restore {
    use super::*;
    #[test]
    fn unwind_preserves_outer_level_and_releases_abandoned_bans() {
        let initial = dartforge_efeitos_nivel();
        let name = b"fixture-unwind";
        // SAFETY: name points to valid bytes during both calls.
        unsafe {
            dartforge_efeitos_antes(name.as_ptr(), name.len() as i64, 1);
            dartforge_efeitos_antes(name.as_ptr(), name.len() as i64, 1);
        }
        dartforge_efeitos_restaurar(initial + 1);
        assert_eq!(dartforge_efeitos_nivel(), initial + 1);
        dartforge_efeitos_restaurar(initial);
        assert_eq!(dartforge_efeitos_nivel(), initial);
        dartforge_arc_collect();
    }
}
