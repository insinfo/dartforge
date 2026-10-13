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
