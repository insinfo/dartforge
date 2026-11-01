//! Memory and time accounting to measure the real cost of compilation.
//!
//! The goal is to answer "how much work was done", not just "how much time
//! passed". A counting allocator reports the process's live bytes and peak
//! live bytes; the phase counters report how many units were reread,
//! reanalyzed and re-emitted. Without these numbers any incremental
//! performance claim would be unverifiable.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

/// Allocator that delegates to the system and keeps live bytes, peak and the
/// number of allocations.
///
/// Install it in the measuring binary with `#[global_allocator]`. The cost is
/// two atomic integers per allocation, enough to compare variants of the
/// compiler itself, but it must not be used in production builds.
///
/// ```
/// # use dartforge_instrument::CountingAllocator;
/// // #[global_allocator]
/// static ALLOCATOR: CountingAllocator = CountingAllocator;
/// let _ = &ALLOCATOR;
/// ```
pub struct CountingAllocator;

/// Updates the peak with the largest value seen so far, without locking the
/// allocator.
fn record(live: usize) {
    let mut peak = PEAK.load(Ordering::Relaxed);
    while live > peak {
        match PEAK.compare_exchange_weak(peak, live, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return,
            Err(observed) => peak = observed,
        }
    }
}

// SAFETY: each method only delegates to `System`, which fulfills the
// GlobalAlloc contract, and updates atomic counters that do not affect the
// pointers.
unsafe impl GlobalAlloc for CountingAllocator {
    /// Delegates to the system and adds the requested size when the allocation
    /// succeeds.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            record(LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size());
        }
        pointer
    }
    /// Returns to the system and subtracts exactly the size recorded at
    /// allocation.
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }
    /// Delegates to the system and adjusts the size difference in a single
    /// operation.
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, new_size) };
        if !result.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            if new_size >= layout.size() {
                let growth = new_size - layout.size();
                record(LIVE.fetch_add(growth, Ordering::Relaxed) + growth);
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        result
    }
}

/// Bytes currently live according to the counting allocator.
pub fn live_bytes() -> usize {
    LIVE.load(Ordering::Relaxed)
}
/// Largest live byte count observed since the last `reset_peak`.
pub fn peak_bytes() -> usize {
    PEAK.load(Ordering::Relaxed)
}
/// Number of successful allocations and reallocations since the process
/// started.
pub fn allocation_count() -> usize {
    ALLOCATIONS.load(Ordering::Relaxed)
}
/// Resets the peak to the current live value before measuring a new
/// operation.
pub fn reset_peak() {
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}

/// CPU time (user + system) consumed so far, split into this process (all of
/// its threads) and its terminated, waited-for children (the Clang and linker
/// processes a native compilation spawns).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuTime {
    pub own: std::time::Duration,
    pub children: std::time::Duration,
}

impl CpuTime {
    /// The CPU time spent between `earlier` and `self`, own and children together.
    ///
    /// ```
    /// # use dartforge_instrument::CpuTime;
    /// use std::time::Duration;
    /// let a = CpuTime { own: Duration::from_secs(1), children: Duration::from_secs(2) };
    /// let b = CpuTime { own: Duration::from_secs(4), children: Duration::from_secs(3) };
    /// assert_eq!(b.since(a), Duration::from_secs(4));
    /// ```
    pub fn since(self, earlier: CpuTime) -> std::time::Duration {
        (self.own + self.children).saturating_sub(earlier.own + earlier.children)
    }
}

/// The current [`CpuTime`], from `getrusage`; zero where it is not available
/// (Windows, WebAssembly), so a report shows `0ns` rather than failing.
///
/// ```
/// let t = dartforge_instrument::cpu_time();
/// let mut x = 0u64;
/// for i in 0..1_000_000u64 { x = x.wrapping_add(std::hint::black_box(i)); }
/// assert!(dartforge_instrument::cpu_time().since(t) >= std::time::Duration::ZERO);
/// ```
pub fn cpu_time() -> CpuTime {
    #[cfg(unix)]
    {
        CpuTime {
            own: rusage(0),
            children: rusage(-1),
        }
    }
    #[cfg(not(unix))]
    {
        CpuTime::default()
    }
}

/// The physical memory of the machine, in bytes (`hw.memsize` on macOS, the
/// pages of `sysconf` on Linux); `None` elsewhere or when the system does not
/// say.
///
/// ```
/// if let Some(bytes) = dartforge_instrument::physical_memory() {
///     assert!(bytes > 0);
/// }
/// ```
pub fn physical_memory() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn sysctlbyname(
                name: *const u8,
                old: *mut u64,
                old_len: *mut usize,
                new: *const u8,
                new_len: usize,
            ) -> i32;
        }
        let mut value: u64 = 0;
        let mut len = std::mem::size_of::<u64>();
        // SAFETY: a NUL-terminated name, an 8-byte output with its length, no
        // new value; `hw.memsize` is a 64-bit integer.
        let r = unsafe {
            sysctlbyname(
                c"hw.memsize".as_ptr().cast(),
                &mut value,
                &mut len,
                std::ptr::null(),
                0,
            )
        };
        (r == 0 && len == std::mem::size_of::<u64>() && value > 0).then_some(value)
    }
    #[cfg(target_os = "linux")]
    {
        unsafe extern "C" {
            fn sysconf(name: i32) -> i64;
        }
        // `_SC_PHYS_PAGES` and `_SC_PAGESIZE` of glibc and musl.
        // SAFETY: `sysconf` only reads system configuration.
        let (pages, size) = unsafe { (sysconf(85), sysconf(30)) };
        (pages > 0 && size > 0).then(|| pages as u64 * size as u64)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// `ru_utime + ru_stime` of `getrusage(who)` (`RUSAGE_SELF` = 0,
/// `RUSAGE_CHILDREN` = -1 on Linux and macOS).
#[cfg(unix)]
fn rusage(who: i32) -> std::time::Duration {
    // `struct rusage` begins with two `struct timeval`; the rest (14 longs) is
    // not read. The buffer is larger than the structure on every Unix.
    #[repr(C, align(8))]
    struct Buffer([u8; 512]);
    unsafe extern "C" {
        fn getrusage(who: i32, usage: *mut Buffer) -> i32;
    }
    let mut buffer = Buffer([0; 512]);
    // SAFETY: `getrusage` writes at most `sizeof(struct rusage)` (144 bytes on
    // 64-bit Linux and macOS) into a buffer of 512 aligned bytes.
    if unsafe { getrusage(who, &mut buffer) } != 0 {
        return std::time::Duration::ZERO;
    }
    let b = &buffer.0;
    let word = |at: usize| i64::from_ne_bytes(b[at..at + 8].try_into().unwrap());
    let half = |at: usize| i32::from_ne_bytes(b[at..at + 4].try_into().unwrap());
    // `timeval` is `{ time_t tv_sec; suseconds_t tv_usec; }`: 16 bytes on both,
    // with a 32-bit `tv_usec` (and padding) on macOS, 64-bit on Linux.
    let usec = |at: usize| {
        if cfg!(target_os = "macos") {
            i64::from(half(at))
        } else {
            word(at)
        }
    };
    let timeval = |at: usize| {
        let s = u64::try_from(word(at)).unwrap_or(0);
        let us = u64::try_from(usec(at + 8)).unwrap_or(0);
        std::time::Duration::from_micros(s * 1_000_000 + us)
    };
    timeval(0) + timeval(16)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The counters only exist when the allocator is installed. The test binary
    // has to install it itself: outside it, the crate is just the allocator's
    // definition and the counters stay at zero.
    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    /// The peak follows a large allocation and follows the live value again
    /// after a reset.
    #[test]
    fn peak_tracks_allocations_and_reset_returns_to_live() {
        reset_peak();
        let base = live_bytes();
        let allocations = allocation_count();
        let block = vec![0u8; 4 * 1024 * 1024];
        assert!(live_bytes() >= base + block.len());
        assert!(peak_bytes() >= base + block.len());
        assert!(allocation_count() > allocations);
        drop(block);
        reset_peak();
        assert!(peak_bytes() <= live_bytes().saturating_add(1024));
    }
}
