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
// * só os pedidos de até [`ALOC_MAIOR`] bytes com alinhamento de até 16
//   passam pelas listas; o resto vai direto ao `System`;
// * o bloco de uma classe é SEMPRE pedido ao `System` com o tamanho da
//   classe e alinhamento 16 — com a lista ligada ou não, e em qualquer
//   thread —, então qualquer bloco de uma classe pode ir à lista de qualquer
//   thread, ou de volta ao `System`, com o mesmo `Layout`;
// * a lista só é usada nas threads que a ligam ([`ligar_cache_de_alocacao`]:
//   a do isolado principal e a de cada `Isolate.spawn`), e cada uma guarda
//   no máximo [`ALOC_LIMITE_POR_CLASSE`] bytes por classe; a thread do
//   isolado a esvazia ao terminar ([`esvaziar_cache_de_alocacao`]). As
//   threads do `dart:io` (eventos, processos) usam o `System` direto;
// * `DARTFORGE_ALOCADOR_SEM_CACHE=1` deixa as listas desligadas (medida e
//   diagnóstico).
//
// Só o executável (as features `aot` e `dll` do crate) instala o alocador;
// o JIT e os testes do crate ficam com o alocador do processo que os hospeda.

/// O passo das classes de tamanho (e o alinhamento de cada bloco).
const ALOC_PASSO: usize = 16;
/// O maior pedido que passa pelas listas.
const ALOC_MAIOR: usize = 512;
/// Quantas classes: 16, 32, …, [`ALOC_MAIOR`].
const ALOC_CLASSES: usize = ALOC_MAIOR / ALOC_PASSO;
/// Quanto cada lista guarda, em bytes, antes de devolver ao `System`.
const ALOC_LIMITE_POR_CLASSE: usize = 64 * 1024;

/// As listas livres da thread: a cabeça de cada classe (o próximo bloco
/// mora na primeira palavra do bloco) e quantos blocos ela tem.
struct CacheDeAlocacao {
    ligado: std::cell::Cell<bool>,
    cabecas: [std::cell::Cell<*mut u8>; ALOC_CLASSES],
    contagens: [std::cell::Cell<u32>; ALOC_CLASSES],
}

thread_local! {
    // `const` e sem `Drop`: o acesso não aloca nem registra destrutor, e
    // vale até o fim da thread (o alocador é chamado de dentro de qualquer
    // código, inclusive dos destrutores de outras variáveis da thread).
    static CACHE_DE_ALOCACAO: CacheDeAlocacao = const {
        CacheDeAlocacao {
            ligado: std::cell::Cell::new(false),
            cabecas: [const { std::cell::Cell::new(std::ptr::null_mut()) }; ALOC_CLASSES],
            contagens: [const { std::cell::Cell::new(0) }; ALOC_CLASSES],
        }
    };
}

/// A classe de tamanho de `l`, se ele passa pelas listas.
#[inline(always)]
fn classe_de_alocacao(l: std::alloc::Layout) -> Option<usize> {
    (l.size() <= ALOC_MAIOR && l.align() <= ALOC_PASSO)
        .then(|| l.size().max(1).div_ceil(ALOC_PASSO) - 1)
}

/// O `Layout` com que todo bloco da classe `c` é pedido ao `System`.
#[inline(always)]
fn layout_da_classe(c: usize) -> std::alloc::Layout {
    // SAFETY: tamanho múltiplo de 16 e não nulo, alinhamento potência de 2.
    unsafe { std::alloc::Layout::from_size_align_unchecked((c + 1) * ALOC_PASSO, ALOC_PASSO) }
}

/// Quantos blocos a lista da classe `c` guarda no máximo.
#[inline(always)]
fn limite_da_classe(c: usize) -> u32 {
    (ALOC_LIMITE_POR_CLASSE / ((c + 1) * ALOC_PASSO)) as u32
}

/// Tira um bloco da lista da classe `c` da thread (nulo: lista vazia ou
/// desligada).
#[inline(always)]
fn tirar_da_lista(c: usize) -> *mut u8 {
    CACHE_DE_ALOCACAO
        .try_with(|k| {
            let cabeca = k.cabecas[c].get();
            if cabeca.is_null() {
                return cabeca;
            }
            // SAFETY: um bloco na lista é um bloco livre da classe `c` (pelo
            // menos 16 bytes, alinhado a 16), com o próximo na 1ª palavra.
            let proximo = unsafe { *(cabeca as *mut *mut u8) };
            k.cabecas[c].set(proximo);
            k.contagens[c].set(k.contagens[c].get() - 1);
            cabeca
        })
        .unwrap_or(std::ptr::null_mut())
}

/// Põe o bloco `p` (livre, da classe `c`) na lista da thread; `false` se a
/// lista está desligada ou cheia.
#[inline(always)]
fn por_na_lista(c: usize, p: *mut u8) -> bool {
    CACHE_DE_ALOCACAO
        .try_with(|k| {
            let n = k.contagens[c].get();
            if !k.ligado.get() || n >= limite_da_classe(c) {
                return false;
            }
            // SAFETY: `p` é um bloco da classe `c` que o chamador liberou.
            unsafe { *(p as *mut *mut u8) = k.cabecas[c].get() };
            k.cabecas[c].set(p);
            k.contagens[c].set(n + 1);
            true
        })
        .unwrap_or(false)
}

/// Liga as listas livres na thread corrente (a de um isolado), salvo com
/// `DARTFORGE_ALOCADOR_SEM_CACHE=1`.
pub fn ligar_cache_de_alocacao() {
    let desligado =
        std::env::var_os("DARTFORGE_ALOCADOR_SEM_CACHE").is_some_and(|v| !v.is_empty() && v != "0");
    let _ = CACHE_DE_ALOCACAO.try_with(|k| k.ligado.set(!desligado));
}

/// Desliga as listas da thread corrente e devolve ao `System` o que elas
/// guardam (o fim da thread de um isolado).
pub fn esvaziar_cache_de_alocacao() {
    let _ = CACHE_DE_ALOCACAO.try_with(|k| {
        k.ligado.set(false);
        for c in 0..ALOC_CLASSES {
            let mut p = k.cabecas[c].replace(std::ptr::null_mut());
            k.contagens[c].set(0);
            while !p.is_null() {
                // SAFETY: como em `tirar_da_lista`; cada bloco veio do
                // `System` com `layout_da_classe(c)`.
                unsafe {
                    let proximo = *(p as *mut *mut u8);
                    std::alloc::GlobalAlloc::dealloc(&std::alloc::System, p, layout_da_classe(c));
                    p = proximo;
                }
            }
        }
    });
}

/// O alocador global do executável (ver o começo deste fragmento).
pub struct AlocadorComCache;

// SAFETY: todo bloco devolvido tem pelo menos o tamanho e o alinhamento
// pedidos (o da classe, ≥ tamanho, alinhado a 16 ≥ alinhamento), e cada
// bloco volta ao `System` com o mesmo `Layout` com que saiu dele.
unsafe impl std::alloc::GlobalAlloc for AlocadorComCache {
    #[inline]
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        match classe_de_alocacao(l) {
            Some(c) => {
                let p = tirar_da_lista(c);
                if !p.is_null() {
                    return p;
                }
                // SAFETY: layout de tamanho não nulo.
                unsafe { std::alloc::System.alloc(layout_da_classe(c)) }
            }
            // SAFETY: o contrato do chamador, repassado.
            None => unsafe { std::alloc::System.alloc(l) },
        }
    }

    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        match classe_de_alocacao(l) {
            Some(c) => {
                if !por_na_lista(c, p) {
                    // SAFETY: o bloco saiu do `System` com o layout da classe.
                    unsafe { std::alloc::System.dealloc(p, layout_da_classe(c)) }
                }
            }
            // SAFETY: o contrato do chamador, repassado.
            None => unsafe { std::alloc::System.dealloc(p, l) },
        }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
        match classe_de_alocacao(l) {
            Some(c) => {
                let p = tirar_da_lista(c);
                if p.is_null() {
                    // SAFETY: layout de tamanho não nulo.
                    return unsafe { std::alloc::System.alloc_zeroed(layout_da_classe(c)) };
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
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, novo: usize) -> *mut u8 {
        // SAFETY: o contrato de `realloc` garante o tamanho novo válido para
        // o alinhamento de `l`.
        let nl = unsafe { std::alloc::Layout::from_size_align_unchecked(novo, l.align()) };
        match (classe_de_alocacao(l), classe_de_alocacao(nl)) {
            // A mesma classe: o bloco já tem o tamanho novo.
            (Some(a), Some(b)) if a == b => p,
            // SAFETY: nenhum dos dois passa pelas listas: o `System` faz.
            (None, None) => unsafe { std::alloc::System.realloc(p, l, novo) },
            _ => {
                // SAFETY: `nl` é válido; os blocos não se sobrepõem.
                unsafe {
                    let q = self.alloc(nl);
                    if !q.is_null() {
                        std::ptr::copy_nonoverlapping(p, q, l.size().min(novo));
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
static ALOCADOR_GLOBAL: AlocadorComCache = AlocadorComCache;
