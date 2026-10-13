//! The executable's allocator with per-thread free lists (`src/allocator.rs`),
//! called directly (the crate's tests do not install it as the global allocator).

use dartforge_runtime::abi::{
    CachingAllocator, flush_allocation_cache, enable_allocation_cache,
};
use std::alloc::{GlobalAlloc, Layout};

/// On a new thread, so that the lists start empty and enabled.
fn in_a_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        enable_allocation_cache();
        f();
        flush_allocation_cache();
    })
    .join()
    .expect("test thread");
}

#[test]
#[allow(unsafe_code)]
fn free_block_returns_through_class_list() {
    in_a_thread(|| {
        let a = CachingAllocator;
        let l = Layout::from_size_align(40, 8).unwrap();
        // SAFETY: valid layouts; each block is freed with its own layout.
        unsafe {
            let p = a.alloc(l);
            assert!(!p.is_null());
            assert_eq!(p as usize % 16, 0, "class block aligned to 16");
            a.dealloc(p, l);
            // 33..=48 bytes: the same class; the block comes off the list.
            let l2 = Layout::from_size_align(48, 16).unwrap();
            let q = a.alloc(l2);
            assert_eq!(p, q);
            // Zeroed when it comes off the list.
            std::ptr::write_bytes(q, 0xAB, 48);
            a.dealloc(q, l2);
            let z = a.alloc_zeroed(l);
            assert_eq!(z, p);
            assert!((0..40).all(|i| *z.add(i) == 0));
            a.dealloc(z, l);
        }
    });
}

#[test]
#[allow(unsafe_code)]
fn realloc_preserves_content_across_classes_and_outside() {
    in_a_thread(|| {
        let a = CachingAllocator;
        // SAFETY: as above; `realloc` receives the block's current layout.
        unsafe {
            let mut l = Layout::from_size_align(8, 8).unwrap();
            let mut p = a.alloc(l);
            for i in 0..8 {
                *p.add(i) = i as u8;
            }
            // Same class (≤ 16), another class, the limit (512) and beyond (> 512).
            for new in [16usize, 100, 512, 4000, 20000, 300, 8] {
                let q = a.realloc(p, l, new);
                assert!(!q.is_null());
                for i in 0..8 {
                    assert_eq!(*q.add(i), i as u8, "realloc to {new}");
                }
                p = q;
                l = Layout::from_size_align(new, 8).unwrap();
            }
            a.dealloc(p, l);
        }
    });
}

#[test]
#[allow(unsafe_code)]
fn full_list_and_large_alignment_go_to_system() {
    in_a_thread(|| {
        let a = CachingAllocator;
        let l = Layout::from_size_align(512, 8).unwrap();
        // SAFETY: as above.
        unsafe {
            // More blocks than the list keeps (64 KiB / 512 = 128): the
            // excess goes back to the system, and all come out distinct.
            let ps: Vec<*mut u8> = (0..300).map(|_| a.alloc(l)).collect();
            let mut uniques = ps.clone();
            uniques.sort();
            uniques.dedup();
            assert_eq!(uniques.len(), ps.len());
            for &p in &ps {
                a.dealloc(p, l);
            }
            let g = Layout::from_size_align(64, 64).unwrap();
            let p = a.alloc(g);
            assert_eq!(p as usize % 64, 0);
            a.dealloc(p, g);
        }
    });
}

#[test]
#[allow(unsafe_code)]
fn block_freed_on_other_thread() {
    let a = CachingAllocator;
    let l = Layout::from_size_align(24, 8).unwrap();
    // SAFETY: the block passes from one thread to the other as an address; each
    // side uses it alone.
    let p = unsafe { a.alloc(l) } as usize;
    in_a_thread(move || {
        let a = CachingAllocator;
        unsafe {
            a.dealloc(p as *mut u8, l);
            let q = a.alloc(l);
            assert_eq!(q as usize, p);
            a.dealloc(q, l);
        }
    });
}
