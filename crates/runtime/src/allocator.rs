// Native runtime: the executable's global allocator, with a per-thread cache (N17).
//
// Every runtime value that is not an object of the space (closures, environments,
// lists, strings, element vectors) is a Rust `Box`/`Vec`, and the collection
// returns each one with `free`. On Windows Rust's `System` is the process heap's
// `HeapAlloc`/`HeapFree` (with `GetProcessHeap` on every call and the heap lock):
// in the HTTP server (`bench/http`) the pair was ~11% of the isolate's active CPU.
// This allocator puts in front of `System` a free list per size class and per
// thread — glibc's *tcache*, or the VM's TLAB for what is not a Dart object: a
// small allocation that finds a block in the list does not call the system, and
// the collection's `free` returns the block to the list.
//
// Contract:
// * only requests of up to [`ALLOC_LARGEST`] bytes with alignment up to 16 go
//   through the lists; the rest goes straight to `System`;
// * a class's block is ALWAYS requested from `System` with the class size and
//   alignment 16 — with the list enabled or not, and on any thread —, so any
//   block of a class may go to the list of any thread, or back to `System`, with
//   the same `Layout`;
// * the list is only used on threads that enable it ([`enable_allocation_cache`]:
//   the main isolate's and that of each `Isolate.spawn`), and each one keeps at
//   most [`ALLOC_LIMIT_PER_CLASS`] bytes per class; the isolate's thread drains it
//   when it ends ([`flush_allocation_cache`]). The `dart:io` threads (events,
//   processes) use `System` directly;
// * `DARTFORGE_ALOCADOR_SEM_CACHE=1` leaves the lists disabled (measurement and
//   diagnosis).
//
// Only the executable (the crate's `aot` and `dll` features) installs the
// allocator; the JIT and the crate's tests keep the allocator of the host process.

/// The step of the size classes (and the alignment of each block).
const ALLOC_STEP: usize = 16;
/// The largest request that goes through the lists.
const ALLOC_LARGEST: usize = 512;
/// How many classes: 16, 32, …, [`ALLOC_LARGEST`].
const ALLOC_CLASSES: usize = ALLOC_LARGEST / ALLOC_STEP;
/// How much each list keeps, in bytes, before returning to `System`.
const ALLOC_LIMIT_PER_CLASS: usize = 64 * 1024;

/// The thread's free lists: the head of each class (the next block lives in the
/// block's first word) and how many blocks it has.
struct AllocationCache {
    enabled_flag: std::cell::Cell<bool>,
    heads: [std::cell::Cell<*mut u8>; ALLOC_CLASSES],
    counts: [std::cell::Cell<u32>; ALLOC_CLASSES],
}

thread_local! {
    // `const` and without `Drop`: access neither allocates nor registers a
    // destructor, and it lasts until the end of the thread (the allocator is
    // called from inside any code, including the destructors of other thread
    // variables).
    static ALLOCATION_CACHE: AllocationCache = const {
        AllocationCache {
            enabled_flag: std::cell::Cell::new(false),
            heads: [const { std::cell::Cell::new(std::ptr::null_mut()) }; ALLOC_CLASSES],
            counts: [const { std::cell::Cell::new(0) }; ALLOC_CLASSES],
        }
    };
}

/// The size class of `l`, if it goes through the lists.
#[inline(always)]
fn allocation_class(l: std::alloc::Layout) -> Option<usize> {
    (l.size() <= ALLOC_LARGEST && l.align() <= ALLOC_STEP)
        .then(|| l.size().max(1).div_ceil(ALLOC_STEP) - 1)
}

/// The `Layout` with which every block of class `c` is requested from `System`.
#[inline(always)]
fn class_layout(c: usize) -> std::alloc::Layout {
    // SAFETY: size is a non-zero multiple of 16, alignment a power of 2.
    unsafe { std::alloc::Layout::from_size_align_unchecked((c + 1) * ALLOC_STEP, ALLOC_STEP) }
}

/// How many blocks the list of class `c` keeps at most.
#[inline(always)]
fn class_limit(c: usize) -> u32 {
    (ALLOC_LIMIT_PER_CLASS / ((c + 1) * ALLOC_STEP)) as u32
}

/// Takes a block from the thread's list of class `c` (null: the list is empty or
/// disabled).
#[inline(always)]
fn take_from_list(c: usize) -> *mut u8 {
    ALLOCATION_CACHE
        .try_with(|k| {
            let head = k.heads[c].get();
            if head.is_null() {
                return head;
            }
            // SAFETY: a block in the list is a free block of class `c` (at least
            // 16 bytes, aligned to 16), with the next one in its first word.
            let next_offset = unsafe { *(head as *mut *mut u8) };
            k.heads[c].set(next_offset);
            k.counts[c].set(k.counts[c].get() - 1);
            head
        })
        .unwrap_or(std::ptr::null_mut())
}

/// Puts block `p` (free, of class `c`) on the thread's list; `false` if the list
/// is disabled or full.
#[inline(always)]
fn put_in_list(c: usize, p: *mut u8) -> bool {
    ALLOCATION_CACHE
        .try_with(|k| {
            let n = k.counts[c].get();
            if !k.enabled_flag.get() || n >= class_limit(c) {
                return false;
            }
            // SAFETY: `p` is a block of class `c` that the caller freed.
            unsafe { *(p as *mut *mut u8) = k.heads[c].get() };
            k.heads[c].set(p);
            k.counts[c].set(n + 1);
            true
        })
        .unwrap_or(false)
}

/// Enables the free lists on the current thread (an isolate's), unless
/// `DARTFORGE_ALOCADOR_SEM_CACHE=1`.
pub fn enable_allocation_cache() {
    let disabled =
        std::env::var_os("DARTFORGE_ALOCADOR_SEM_CACHE").is_some_and(|v| !v.is_empty() && v != "0");
    let _ = ALLOCATION_CACHE.try_with(|k| k.enabled_flag.set(!disabled));
}

/// Disables the current thread's lists and returns what they keep to `System`
/// (the end of an isolate's thread).
pub fn flush_allocation_cache() {
    let _ = ALLOCATION_CACHE.try_with(|k| {
        k.enabled_flag.set(false);
        for c in 0..ALLOC_CLASSES {
            let mut p = k.heads[c].replace(std::ptr::null_mut());
            k.counts[c].set(0);
            while !p.is_null() {
                // SAFETY: as in `take_from_list`; every block came from `System`
                // with `class_layout(c)`.
                unsafe {
                    let next_offset = *(p as *mut *mut u8);
                    std::alloc::GlobalAlloc::dealloc(&std::alloc::System, p, class_layout(c));
                    p = next_offset;
                }
            }
        }
    });
}

/// The executable's global allocator (see the start of this fragment).
pub struct CachingAllocator;

// SAFETY: every returned block has at least the requested size and alignment (the
// class's, ≥ size, aligned to 16 ≥ alignment), and each block goes back to
// `System` with the same `Layout` it came out with.
unsafe impl std::alloc::GlobalAlloc for CachingAllocator {
    #[inline]
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        match allocation_class(l) {
            Some(c) => {
                let p = take_from_list(c);
                if !p.is_null() {
                    return p;
                }
                // SAFETY: layout with a non-zero size.
                unsafe { std::alloc::System.alloc(class_layout(c)) }
            }
            // SAFETY: the caller's contract, passed on.
            None => unsafe { std::alloc::System.alloc(l) },
        }
    }

    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        match allocation_class(l) {
            Some(c) => {
                if !put_in_list(c, p) {
                    // SAFETY: the block came from `System` with the class layout.
                    unsafe { std::alloc::System.dealloc(p, class_layout(c)) }
                }
            }
            // SAFETY: the caller's contract, passed on.
            None => unsafe { std::alloc::System.dealloc(p, l) },
        }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
        match allocation_class(l) {
            Some(c) => {
                let p = take_from_list(c);
                if p.is_null() {
                    // SAFETY: layout with a non-zero size.
                    return unsafe { std::alloc::System.alloc_zeroed(class_layout(c)) };
                }
                // SAFETY: the block has at least `l.size()` bytes.
                unsafe { std::ptr::write_bytes(p, 0, l.size()) };
                p
            }
            // SAFETY: the caller's contract, passed on.
            None => unsafe { std::alloc::System.alloc_zeroed(l) },
        }
    }

    #[inline]
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, new: usize) -> *mut u8 {
        // SAFETY: the `realloc` contract guarantees the new size is valid for the
        // alignment of `l`.
        let nl = unsafe { std::alloc::Layout::from_size_align_unchecked(new, l.align()) };
        match (allocation_class(l), allocation_class(nl)) {
            // The same class: the block already has the new size.
            (Some(a), Some(b)) if a == b => p,
            // SAFETY: neither goes through the lists: `System` does it.
            (None, None) => unsafe { std::alloc::System.realloc(p, l, new) },
            _ => {
                // SAFETY: `nl` is valid; the blocks do not overlap.
                unsafe {
                    let q = self.alloc(nl);
                    if !q.is_null() {
                        std::ptr::copy_nonoverlapping(p, q, l.size().min(new));
                        self.dealloc(p, l);
                    }
                    q
                }
            }
        }
    }
}

/// The executable's allocator: only in the `staticlib` profiles (the JIT and the
/// tests use the one of their host process).
#[cfg(not(dartforge_runtime_embutido))]
#[global_allocator]
static GLOBAL_ALLOCATOR: CachingAllocator = CachingAllocator;
