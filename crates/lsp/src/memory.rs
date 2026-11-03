//! The live heap of the server process, for the memory log.
//!
//! With `DARTFORGE_LSP_MEMORY=1`, [`log`] prints to `stderr` the bytes the
//! system allocator has handed out and not taken back at a point of the server
//! (the typed worker logs before and after each analysis, and the analysis
//! itself at its cancellation points). It is how `docs/LSP.md` ("Memória")
//! tells retention from allocator behavior: on `new_sali` the heap in use
//! after the workspace analysis is ~17–27 MiB while the RSS stays at the
//! analysis peak, because the freed pages stay with the allocator.

/// The bytes the system allocator has handed out and not taken back (live
/// heap), when the platform reports it.
pub(crate) fn heap_in_use() -> Option<usize> {
    platform::heap_in_use()
}

/// Gives the free pages the system allocator keeps back to the system, when
/// the platform allows it, and returns how many bytes it gave back. The
/// resident calls it after each analysis (`DARTFORGE_LSP_RELIEF=0` turns it
/// off): its live data stays flat over a long session, and what grew was the
/// pages the allocator kept after the peaks (docs/LSP.md, "Décima quinta
/// rodada").
pub(crate) fn release_free_pages() -> usize {
    platform::release_free_pages()
}

/// The resident gives free pages back after each analysis
/// (`DARTFORGE_LSP_RELIEF`, on by default).
pub(crate) fn relief() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("DARTFORGE_LSP_RELIEF").map_or(true, |v| v.trim() != "0"))
}

/// The memory log is on (`DARTFORGE_LSP_MEMORY=1`).
pub(crate) fn logging() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("DARTFORGE_LSP_MEMORY").is_ok_and(|v| v != "0" && !v.is_empty())
    })
}

/// The hand-off trace is on (`DARTFORGE_LSP_TRACE=1`).
pub(crate) fn tracing() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("DARTFORGE_LSP_TRACE").is_ok_and(|v| v != "0" && !v.is_empty())
    })
}

/// With the trace on, prints `[trace] <µs since the epoch> <event>` to
/// `stderr`: the moments a change goes from one thread or process to the
/// next (server, worker, resident), on a clock every process shares, to
/// measure the time each hand-off waits.
pub(crate) fn trace(event: &str) {
    if !tracing() {
        return;
    }
    let micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_micros());
    eprintln!("[trace] {micros} {event}");
}

/// With the memory log on, prints the live heap at `label` to `stderr`.
pub(crate) fn log(label: &str) {
    if !logging() {
        return;
    }
    match heap_in_use() {
        Some(bytes) => eprintln!(
            "[dartforge-lsp] memory {label}: {:.1} MiB in use",
            bytes as f64 / (1024.0 * 1024.0)
        ),
        None => eprintln!("[dartforge-lsp] memory {label}: (not reported on this platform)"),
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod platform {
    use std::ffi::c_void;

    /// `malloc_statistics_t` of `<malloc/malloc.h>`.
    #[repr(C)]
    #[derive(Default)]
    struct MallocStatistics {
        blocks_in_use: u32,
        size_in_use: usize,
        max_size_in_use: usize,
        size_allocated: usize,
    }

    unsafe extern "C" {
        fn malloc_zone_statistics(zone: *mut c_void, stats: *mut MallocStatistics);
        fn malloc_zone_pressure_relief(zone: *mut c_void, goal: usize) -> usize;
    }

    pub(super) fn release_free_pages() -> usize {
        // SAFETY: a null zone asks every registered zone to give back its free
        // pages, and a zero goal means as many as it can; the call touches no
        // memory of ours.
        unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) }
    }

    pub(super) fn heap_in_use() -> Option<usize> {
        let mut stats = MallocStatistics::default();
        // SAFETY: a null zone asks for the sum over every registered zone, and
        // `stats` is a valid, exclusively borrowed `malloc_statistics_t` that the
        // call only writes.
        unsafe { malloc_zone_statistics(std::ptr::null_mut(), &mut stats) };
        Some(stats.size_in_use)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub(super) fn heap_in_use() -> Option<usize> {
        None
    }

    pub(super) fn release_free_pages() -> usize {
        0
    }
}
