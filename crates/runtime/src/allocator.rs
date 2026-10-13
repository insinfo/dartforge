// Runtime nativo: o alocador global do executável, com cache por thread (N17).
//
// Todo valor do runtime que não é objeto do espaço (closures, ambientes,
// listas, strings, os vetores de elementos) é um `Box`/`Vec` do Rust, e a
// coleta devolve cada um com `free`. No Windows o `System` do Rust é o
// `HeapAlloc`/`HeapFree` do heap do processo (com o `GetProcessHeap` a cada
// chamada e a trava do heap): no servidor HTTP (`bench/http`) o par era ~11%
// da CPU ativa do isolado. Este alocador põe na frente do `System` uma lista
// livre por classe de tamanho e por thread — o *tcache* da glibc, ou a TLAB
// da VM para o que não é objeto Dart: a alocação pequena que acha bloco na
// lista não chama o sistema, e o `free` da coleta devolve o bloco à lista.
//
// Contrato:
// * só os pedidos de até [`ALLOC_LARGEST`] bytes com alinhamento de até 16
//   passam pelas listas; o resto vai direto ao `System`;
// * o bloco de uma classe é SEMPRE pedido ao `System` com o tamanho da
//   classe e alinhamento 16 — com a lista ligada ou não, e em qualquer
//   thread —, então qualquer bloco de uma classe pode ir à lista de qualquer
//   thread, ou de volta ao `System`, com o mesmo `Layout`;
// * a lista só é usada nas threads que a ligam ([`enable_allocation_cache`]:
//   a do isolado principal e a de cada `Isolate.spawn`), e cada uma guarda
//   no máximo [`ALLOC_LIMIT_PER_CLASS`] bytes por classe; a thread do
//   isolado a esvazia ao terminar ([`flush_allocation_cache`]). As
//   threads do `dart:io` (eventos, processos) usam o `System` direto;
// * `DARTFORGE_ALOCADOR_SEM_CACHE=1` deixa as listas desligadas (medida e
//   diagnóstico).
//
// Só o executável (as features `aot` e `dll` do crate) instala o alocador;
// o JIT e os testes do crate ficam com o alocador do processo que os hospeda.

/// O passo das classes de tamanho (e o alinhamento de cada bloco).
const ALLOC_STEP: usize = 16;
/// O maior pedido que passa pelas listas.
const ALLOC_LARGEST: usize = 512;
/// Quantas classes: 16, 32, …, [`ALLOC_LARGEST`].
const ALLOC_CLASSES: usize = ALLOC_LARGEST / ALLOC_STEP;
/// Quanto cada lista guarda, em bytes, antes de devolver ao `System`.
const ALLOC_LIMIT_PER_CLASS: usize = 64 * 1024;

/// As listas livres da thread: a cabeça de cada classe (o próximo bloco
/// mora na primeira palavra do bloco) e quantos blocos ela tem.
struct AllocationCache {
    enabled_flag: std::cell::Cell<bool>,
    heads: [std::cell::Cell<*mut u8>; ALLOC_CLASSES],
    counts: [std::cell::Cell<u32>; ALLOC_CLASSES],
}

thread_local! {
    // `const` e sem `Drop`: o acesso não aloca nem registra destrutor, e
    // vale até o fim da thread (o alocador é chamado de dentro de qualquer
    // código, inclusive dos destrutores de outras variáveis da thread).
    static ALLOCATION_CACHE: AllocationCache = const {
        AllocationCache {
            enabled_flag: std::cell::Cell::new(false),
            heads: [const { std::cell::Cell::new(std::ptr::null_mut()) }; ALLOC_CLASSES],
            counts: [const { std::cell::Cell::new(0) }; ALLOC_CLASSES],
        }
    };
}

/// A classe de tamanho de `l`, se ele passa pelas listas.
#[inline(always)]
fn allocation_class(l: std::alloc::Layout) -> Option<usize> {
    (l.size() <= ALLOC_LARGEST && l.align() <= ALLOC_STEP)
        .then(|| l.size().max(1).div_ceil(ALLOC_STEP) - 1)
}

/// O `Layout` com que todo bloco da classe `c` é pedido ao `System`.
#[inline(always)]
fn class_layout(c: usize) -> std::alloc::Layout {
    // SAFETY: tamanho múltiplo de 16 e não nulo, alinhamento potência de 2.
    unsafe { std::alloc::Layout::from_size_align_unchecked((c + 1) * ALLOC_STEP, ALLOC_STEP) }
}

/// Quantos blocos a lista da classe `c` guarda no máximo.
#[inline(always)]
fn class_limit(c: usize) -> u32 {
    (ALLOC_LIMIT_PER_CLASS / ((c + 1) * ALLOC_STEP)) as u32
}

/// Tira um bloco da lista da classe `c` da thread (nulo: lista vazia ou
/// desligada).
#[inline(always)]
fn take_from_list(c: usize) -> *mut u8 {
    ALLOCATION_CACHE
        .try_with(|k| {
            let head = k.heads[c].get();
            if head.is_null() {
                return head;
            }
            // SAFETY: um bloco na lista é um bloco livre da classe `c` (pelo
            // menos 16 bytes, alinhado a 16), com o próximo na 1ª palavra.
            let next_offset = unsafe { *(head as *mut *mut u8) };
            k.heads[c].set(next_offset);
            k.counts[c].set(k.counts[c].get() - 1);
            head
        })
        .unwrap_or(std::ptr::null_mut())
}

/// Põe o bloco `p` (livre, da classe `c`) na lista da thread; `false` se a
/// lista está desligada ou cheia.
#[inline(always)]
fn put_in_list(c: usize, p: *mut u8) -> bool {
    ALLOCATION_CACHE
        .try_with(|k| {
            let n = k.counts[c].get();
            if !k.enabled_flag.get() || n >= class_limit(c) {
                return false;
            }
            // SAFETY: `p` é um bloco da classe `c` que o chamador liberou.
            unsafe { *(p as *mut *mut u8) = k.heads[c].get() };
            k.heads[c].set(p);
            k.counts[c].set(n + 1);
            true
        })
        .unwrap_or(false)
}

/// Liga as listas livres na thread corrente (a de um isolado), salvo com
/// `DARTFORGE_ALOCADOR_SEM_CACHE=1`.
pub fn enable_allocation_cache() {
    let disabled =
        std::env::var_os("DARTFORGE_ALOCADOR_SEM_CACHE").is_some_and(|v| !v.is_empty() && v != "0");
    let _ = ALLOCATION_CACHE.try_with(|k| k.enabled_flag.set(!disabled));
}

/// Desliga as listas da thread corrente e devolve ao `System` o que elas
/// guardam (o fim da thread de um isolado).
pub fn flush_allocation_cache() {
    let _ = ALLOCATION_CACHE.try_with(|k| {
        k.enabled_flag.set(false);
        for c in 0..ALLOC_CLASSES {
            let mut p = k.heads[c].replace(std::ptr::null_mut());
            k.counts[c].set(0);
            while !p.is_null() {
                // SAFETY: como em `take_from_list`; cada bloco veio do
                // `System` com `class_layout(c)`.
                unsafe {
                    let next_offset = *(p as *mut *mut u8);
                    std::alloc::GlobalAlloc::dealloc(&std::alloc::System, p, class_layout(c));
                    p = next_offset;
                }
            }
        }
    });
}

/// O alocador global do executável (ver o começo deste fragmento).
pub struct CachingAllocator;

// SAFETY: todo bloco devolvido tem pelo menos o tamanho e o alinhamento
// pedidos (o da classe, ≥ tamanho, alinhado a 16 ≥ alinhamento), e cada
// bloco volta ao `System` com o mesmo `Layout` com que saiu dele.
unsafe impl std::alloc::GlobalAlloc for CachingAllocator {
    #[inline]
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        match allocation_class(l) {
            Some(c) => {
                let p = take_from_list(c);
                if !p.is_null() {
                    return p;
                }
                // SAFETY: layout de tamanho não nulo.
                unsafe { std::alloc::System.alloc(class_layout(c)) }
            }
            // SAFETY: o contrato do chamador, repassado.
            None => unsafe { std::alloc::System.alloc(l) },
        }
    }

    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        match allocation_class(l) {
            Some(c) => {
                if !put_in_list(c, p) {
                    // SAFETY: o bloco saiu do `System` com o layout da classe.
                    unsafe { std::alloc::System.dealloc(p, class_layout(c)) }
                }
            }
            // SAFETY: o contrato do chamador, repassado.
            None => unsafe { std::alloc::System.dealloc(p, l) },
        }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
        match allocation_class(l) {
            Some(c) => {
                let p = take_from_list(c);
                if p.is_null() {
                    // SAFETY: layout de tamanho não nulo.
                    return unsafe { std::alloc::System.alloc_zeroed(class_layout(c)) };
                }
                // SAFETY: o bloco tem pelo menos `l.size()` bytes.
                unsafe { std::ptr::write_bytes(p, 0, l.size()) };
                p
            }
            // SAFETY: o contrato do chamador, repassado.
            None => unsafe { std::alloc::System.alloc_zeroed(l) },
        }
    }

    #[inline]
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, new: usize) -> *mut u8 {
        // SAFETY: o contrato de `realloc` garante o tamanho novo válido para
        // o alinhamento de `l`.
        let nl = unsafe { std::alloc::Layout::from_size_align_unchecked(new, l.align()) };
        match (allocation_class(l), allocation_class(nl)) {
            // A mesma classe: o bloco já tem o tamanho novo.
            (Some(a), Some(b)) if a == b => p,
            // SAFETY: nenhum dos dois passa pelas listas: o `System` faz.
            (None, None) => unsafe { std::alloc::System.realloc(p, l, new) },
            _ => {
                // SAFETY: `nl` é válido; os blocos não se sobrepõem.
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

/// O alocador do executável: só nos perfis da `staticlib` (o JIT e os
/// testes usam o do processo que os hospeda).
#[cfg(not(dartforge_runtime_embutido))]
#[global_allocator]
static GLOBAL_ALLOCATOR: CachingAllocator = CachingAllocator;
