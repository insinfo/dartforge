// Owner-isolate automatic-GC policy. Scheduler hooks never execute Dart while
// holding HEAP and must recheck ready work after collecting finalizers.

/// Whether this runtime exposes bounded automatic-GC policy controls.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_gc_policy_supported() -> u8 {
    1
}

/// Monotonic microseconds elapsed since this owner's heap was initialized.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_gc_policy_elapsed_micros() -> i64 {
    HEAP.with(|h| h.borrow().gc_policy_elapsed_micros())
}

/// Arms a bounded deferral: 1 accepted, 0 debt must be serviced, -1 invalid.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_gc_policy_defer(
    duration_micros: i64,
    max_extra_bytes: i64,
) -> i64 {
    if duration_micros <= 0 || max_extra_bytes <= 0 {
        return -1;
    }
    let Ok(budget) = usize::try_from(max_extra_bytes) else {
        return -1;
    };
    let status = HEAP.with(|h| {
        h.borrow_mut().gc_policy_defer(
            std::time::Duration::from_micros(duration_micros as u64),
            budget,
        )
    });
    native_pump_signal_current();
    status
}

/// Ends a window without erasing pending collection debt or an idle request.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_gc_policy_cancel() {
    HEAP.with(|h| h.borrow_mut().gc_policy_cancel());
    native_pump_signal_current();
}

/// Requests collection at genuine owner-loop idleness, after any live window.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_gc_policy_request_idle() {
    HEAP.with(|h| h.borrow_mut().gc_policy_request_idle());
    native_pump_signal_current();
}

/// Cumulative collection count, including explicit and authoritative stress GC.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_gc_policy_collections() -> i64 {
    HEAP.with(|h| h.borrow().stats().collections.min(i64::MAX as u64) as i64)
}

/// A successful collection may have queued finalizers: restart scheduling.
fn gc_policy_collect_if_idle() -> bool {
    HEAP.with(|h| h.borrow_mut().gc_policy_collect_if_idle())
}

/// Eligibility deadline for an explicit idle request; does not keep an isolate alive.
fn gc_policy_idle_deadline() -> Option<std::time::Instant> {
    HEAP.with(|h| h.borrow().gc_policy_idle_deadline())
}
