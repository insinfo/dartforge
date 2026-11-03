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

/// The bytes of a file, read once and dropped after use: on Unix a
/// read-only mapping (its pages are the file's, and leave no free pages with
/// the allocator), elsewhere a read into the heap. The reference index's
/// cache (~70 MB on `new_sali/frontend`) read into the heap left the
/// server's footprint ~150 MiB above its live data (docs/LSP.md, "Décima
/// sétima rodada").
pub(crate) enum FileBytes {
    Heap(Vec<u8>),
    #[cfg(unix)]
    Mapped(mapping::Mapping),
}

impl std::ops::Deref for FileBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            FileBytes::Heap(v) => v,
            #[cfg(unix)]
            FileBytes::Mapped(m) => m.bytes(),
        }
    }
}

/// The bytes of the file at `path` ([`FileBytes`]).
pub(crate) fn file_bytes(path: &std::path::Path) -> Option<FileBytes> {
    #[cfg(unix)]
    if let Some(m) = mapping::Mapping::of(path) {
        return Some(FileBytes::Mapped(m));
    }
    std::fs::read(path).ok().map(FileBytes::Heap)
}

#[cfg(unix)]
#[allow(unsafe_code)]
pub(crate) mod mapping {
    use std::ffi::c_void;

    unsafe extern "C" {
        fn mmap(
            addr: *mut c_void,
            len: usize,
            prot: i32,
            flags: i32,
            fd: i32,
            off: i64,
        ) -> *mut c_void;
        fn munmap(addr: *mut c_void, len: usize) -> i32;
    }

    const PROT_READ: i32 = 1;
    const MAP_PRIVATE: i32 = 2;

    /// A read-only private mapping of a whole file.
    pub(crate) struct Mapping {
        at: *mut c_void,
        len: usize,
    }

    // SAFETY: the mapping is read-only and owned; nothing else unmaps it.
    unsafe impl Send for Mapping {}

    impl Mapping {
        /// Maps the file at `path`; `None` when it is empty or cannot be mapped
        /// (the caller reads it instead).
        pub(crate) fn of(path: &std::path::Path) -> Option<Mapping> {
            use std::os::fd::AsRawFd;
            let file = std::fs::File::open(path).ok()?;
            let len = usize::try_from(file.metadata().ok()?.len()).ok()?;
            if len == 0 {
                return None;
            }
            // SAFETY: a fresh read-only private mapping of an open descriptor; the
            // mapping outlives the descriptor, which may close after the call.
            let at = unsafe {
                mmap(
                    std::ptr::null_mut(),
                    len,
                    PROT_READ,
                    MAP_PRIVATE,
                    file.as_raw_fd(),
                    0,
                )
            };
            if at.is_null() || at as isize == -1 {
                return None;
            }
            Some(Mapping { at, len })
        }

        pub(crate) fn bytes(&self) -> &[u8] {
            // SAFETY: `at` maps `len` readable bytes until `drop`. A file changed
            // under the mapping changes the bytes, which the decoder checks.
            unsafe { std::slice::from_raw_parts(self.at.cast::<u8>(), self.len) }
        }
    }

    impl Drop for Mapping {
        fn drop(&mut self) {
            // SAFETY: unmaps exactly what `of` mapped, once.
            unsafe {
                munmap(self.at, self.len);
            }
        }
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
