//! O alocador com listas livres por thread do executável (`src/alocador.rs`),
//! chamado direto (os testes do crate não o instalam como alocador global).

use dartforge_runtime::abi::{
    AlocadorComCache, esvaziar_cache_de_alocacao, ligar_cache_de_alocacao,
};
use std::alloc::{GlobalAlloc, Layout};

/// Numa thread nova, para as listas começarem vazias e ligadas.
fn numa_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        ligar_cache_de_alocacao();
        f();
        esvaziar_cache_de_alocacao();
    })
    .join()
    .expect("thread do teste");
}

#[test]
#[allow(unsafe_code)]
fn bloco_livre_volta_pela_lista_da_classe() {
    numa_thread(|| {
        let a = AlocadorComCache;
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
fn realloc_preserva_o_conteudo_entre_classes_e_para_fora() {
    numa_thread(|| {
        let a = AlocadorComCache;
        // SAFETY: como acima; `realloc` recebe o layout vigente do bloco.
        unsafe {
            let mut l = Layout::from_size_align(8, 8).unwrap();
            let mut p = a.alloc(l);
            for i in 0..8 {
                *p.add(i) = i as u8;
            }
            // Mesma classe (≤ 16), outra classe, o limite (512) e fora (> 512).
            for novo in [16usize, 100, 512, 4000, 20000, 300, 8] {
                let q = a.realloc(p, l, novo);
                assert!(!q.is_null());
                for i in 0..8 {
                    assert_eq!(*q.add(i), i as u8, "realloc para {novo}");
                }
                p = q;
                l = Layout::from_size_align(novo, 8).unwrap();
            }
            a.dealloc(p, l);
        }
    });
}

#[test]
#[allow(unsafe_code)]
fn lista_cheia_e_alinhamento_grande_vao_ao_sistema() {
    numa_thread(|| {
        let a = AlocadorComCache;
        let l = Layout::from_size_align(512, 8).unwrap();
        // SAFETY: como acima.
        unsafe {
            // Mais blocos do que a lista guarda (64 KiB / 512 = 128): o
            // excedente volta ao sistema, e todos saem distintos.
            let ps: Vec<*mut u8> = (0..300).map(|_| a.alloc(l)).collect();
            let mut unicos = ps.clone();
            unicos.sort();
            unicos.dedup();
            assert_eq!(unicos.len(), ps.len());
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
fn bloco_liberado_em_outra_thread() {
    let a = AlocadorComCache;
    let l = Layout::from_size_align(24, 8).unwrap();
    // SAFETY: o bloco passa de uma thread à outra como endereço; cada lado o
    // usa sozinho.
    let p = unsafe { a.alloc(l) } as usize;
    numa_thread(move || {
        let a = AlocadorComCache;
        unsafe {
            a.dealloc(p as *mut u8, l);
            let q = a.alloc(l);
            assert_eq!(q as usize, p);
            a.dealloc(q, l);
        }
    });
}
