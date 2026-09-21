//! Contabilidade de memória e tempo para medir o custo real da compilação.
//!
//! O objetivo é responder "quanto trabalho foi feito", não apenas "quanto tempo
//! passou". Um alocador contador informa bytes vivos e pico de bytes vivos do
//! processo; os contadores de fase informam quantas unidades foram relidas,
//! reanalisadas e reemitidas. Sem esses números qualquer afirmação de
//! desempenho incremental seria não verificável.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

/// Alocador que delega ao sistema e mantém bytes vivos, pico e número de alocações.
///
/// Instale-o no binário de medição com `#[global_allocator]`. O custo é de dois
/// inteiros atômicos por alocação, suficiente para comparar variantes do próprio
/// compilador, mas não deve ser usado em builds de produção.
///
/// ```
/// # use dartforge_instrument::CountingAllocator;
/// // #[global_allocator]
/// static ALOCADOR: CountingAllocator = CountingAllocator;
/// let _ = &ALOCADOR;
/// ```
pub struct CountingAllocator;

/// Atualiza o pico com o maior valor já observado, sem travar o alocador.
fn record(live: usize) {
    let mut peak = PEAK.load(Ordering::Relaxed);
    while live > peak {
        match PEAK.compare_exchange_weak(peak, live, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return,
            Err(observed) => peak = observed,
        }
    }
}

// SAFETY: cada método apenas delega a `System`, que cumpre o contrato de
// GlobalAlloc, e atualiza contadores atômicos que não afetam os ponteiros.
unsafe impl GlobalAlloc for CountingAllocator {
    /// Delega ao sistema e soma o tamanho pedido quando a alocação tem sucesso.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            record(LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size());
        }
        pointer
    }
    /// Devolve ao sistema e subtrai exatamente o tamanho registrado na alocação.
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }
    /// Delega ao sistema e ajusta a diferença de tamanho em uma única operação.
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

/// Bytes atualmente vivos segundo o alocador contador.
pub fn live_bytes() -> usize {
    LIVE.load(Ordering::Relaxed)
}
/// Maior valor de bytes vivos observado desde o último `reset_peak`.
pub fn peak_bytes() -> usize {
    PEAK.load(Ordering::Relaxed)
}
/// Número de alocações e realocações bem-sucedidas desde o início do processo.
pub fn allocation_count() -> usize {
    ALLOCATIONS.load(Ordering::Relaxed)
}
/// Reinicia o pico para o valor vivo atual antes de medir uma nova operação.
pub fn reset_peak() {
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Os contadores só existem quando o alocador está instalado. O binário de
    // teste precisa instalá-lo por conta própria: fora dele, o crate é apenas a
    // definição do alocador e os contadores permanecem zerados.
    #[global_allocator]
    static ALOCADOR: CountingAllocator = CountingAllocator;

    /// O pico acompanha uma alocação grande e volta a acompanhar o vivo após reset.
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
