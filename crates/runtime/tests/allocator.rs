//! O alocador com listas livres por thread do executável (`src/alocador.rs`),
//! chamado direto (os testes do crate não o instalam como alocador global).

use dartforge_runtime::abi::{
    CachingAllocator, flush_allocation_cache, enable_allocation_cache,
};
use std::alloc::{GlobalAlloc, Layout};

/// Numa thread nova, para as listas começarem vazias e ligadas.
fn in_a_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        enable_allocation_cache();
        f();
        flush_allocation_cache();
    })
    .join()
    .expect("thread do teste");
}

#[test]
#[allow(unsafe_code)]
fn free_block_returns_through_class_list() {
    in_a_thread(|| {
        let a = CachingAllocator;
        let l = Layout::from_size_align(40, 8).unwrap();
        // SAFETY: layouts válidos; cada bloco é liberado com o layout dele.
        unsafe {
            let p = a.alloc(l);
            assert!(!p.is_null());
            assert_eq!(p as usize % 16, 0, "bloco de classe alinhado a 16");
            a.dealloc(p, l);
            // 33..=48 bytes: a mesma classe; o bloco sai da lista.
            let l2 = Layout::from_size_align(48, 16).unwrap();
            let q = a.alloc(l2);
            assert_eq!(p, q);
            // Zerado ao sair da lista.
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
        // SAFETY: como acima; `realloc` recebe o layout vigente do bloco.
        unsafe {
            let mut l = Layout::from_size_align(8, 8).unwrap();
            let mut p = a.alloc(l);
            for i in 0..8 {
                *p.add(i) = i as u8;
            }
            // Mesma classe (≤ 16), outra classe, o limite (512) e fora (> 512).
            for new in [16usize, 100, 512, 4000, 20000, 300, 8] {
                let q = a.realloc(p, l, new);
                assert!(!q.is_null());
                for i in 0..8 {
                    assert_eq!(*q.add(i), i as u8, "realloc para {new}");
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
        // SAFETY: como acima.
        unsafe {
            // Mais blocos do que a lista guarda (64 KiB / 512 = 128): o
            // excedente volta ao sistema, e todos saem distintos.
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
    // SAFETY: o bloco passa de uma thread à outra como endereço; cada lado o
    // usa sozinho.
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
