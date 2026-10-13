// Runtime nativo: protocolo de raízes do coletor (frames, globais, coleta).

/// Executa `f` com `handles` enraizados num frame temporário (G6).
///
/// Extern que aloca mais de uma vez: o primeiro objeto só é referenciado
/// por uma variável do Rust enquanto o segundo é alocado, e essa alocação
/// pode coletar. O frame é o mesmo protocolo do código gerado.
fn with_roots<R>(handles: &[i64], f: impl FnOnce() -> R) -> R {
    let frame = HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let frame = h.push_frame_with_slots(handles.len());
        for (i, &x) in handles.iter().enumerate() {
            h.set_root(frame, i, x);
        }
        frame
    });
    let r = f();
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    r
}

/// O endereço do contexto da thread corrente (`heap::Context`): a exceção
/// pendente e o topo da pilha-sombra, que o código gerado lê e grava sem
/// chamada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_contexto() -> *const crate::heap::Context {
    crate::heap::CONTEXTO.with(|c| {
        if c.stack_limit.get() == 0 {
            c.stack_limit.set(thread_stack_limit());
        }
        c as *const crate::heap::Context
    })
}

/// Folga entre o limite que o código gerado confere e o fim da pilha da
/// thread: o que o runtime ainda usa para montar e lançar o
/// `StackOverflowError` (o rastro, a alocação) e para as chamadas nativas
/// de uma função que passou do prólogo.
const STACK_SLACK: usize = 256 * 1024;

/// O endereço mais baixo que a pilha da thread corrente pode usar, mais a
/// [`STACK_SLACK`]; 1 quando o sistema não diz (sem conferência).
#[allow(unsafe_code)]
fn thread_stack_limit() -> usize {
    let base = stack_base();
    // A sabotagem `folga` (D8, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    // §7.3): 4 KiB não bastam para montar e lançar o `StackOverflowError`.
    let folga = if crate::heap::sabotage("folga") { 4 * 1024 } else { STACK_SLACK };
    if base == 0 { 1 } else { base + folga }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn stack_base() -> usize {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
    }
    let (mut low, mut high) = (0usize, 0usize);
    // SAFETY: dois ponteiros para `usize` locais.
    unsafe { GetCurrentThreadStackLimits(&mut low, &mut high) };
    low
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn stack_base() -> usize {
    // `pthread_attr_t` tem 56 bytes no glibc x86-64 e 64 no aarch64.
    #[repr(C, align(8))]
    struct Atributos([u8; 64]);
    unsafe extern "C" {
        fn pthread_self() -> usize;
        fn pthread_getattr_np(t: usize, a: *mut Atributos) -> i32;
        fn pthread_attr_getstack(a: *const Atributos, address: *mut usize, size: *mut usize) -> i32;
        fn pthread_attr_destroy(a: *mut Atributos) -> i32;
    }
    let mut a = Atributos([0; 64]);
    let (mut address, mut size) = (0usize, 0usize);
    // SAFETY: atributos locais, iniciados pelo `pthread_getattr_np` e
    // destruídos no fim.
    unsafe {
        if pthread_getattr_np(pthread_self(), &mut a) != 0 {
            return 0;
        }
        let ok = pthread_attr_getstack(&a, &mut address, &mut size) == 0;
        pthread_attr_destroy(&mut a);
        if ok { address } else { 0 }
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn stack_base() -> usize {
    unsafe extern "C" {
        fn pthread_self() -> usize;
        fn pthread_get_stackaddr_np(t: usize) -> usize;
        fn pthread_get_stacksize_np(t: usize) -> usize;
    }
    // SAFETY: consultas da thread corrente; o endereço é o topo da pilha.
    unsafe {
        let t = pthread_self();
        pthread_get_stackaddr_np(t).saturating_sub(pthread_get_stacksize_np(t))
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn stack_base() -> usize {
    0
}

/// O prólogo de uma função achou o quadro abaixo do limite da pilha
/// (`Context::stack_limit`): lança o `StackOverflowError` do SDK, e a
/// função volta com a exceção pendente, como depois de uma chamada que
/// lançou. O limite é desligado enquanto o erro é montado (o rastro e a
/// alocação usam a folga) e religado depois.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_estouro_de_pilha() {
    let limit = crate::heap::CONTEXTO.with(|c| c.stack_limit.replace(1));
    let error = dartforge_stack_overflow_error_new();
    if dartforge_exception_pending() == 0 {
        with_roots(&[error], || dartforge_exception_throw(error, 3));
    }
    crate::heap::CONTEXTO.with(|c| c.stack_limit.set(limit));
}

/// Valor corrente de um global `Ref` do programa, mantido como raiz permanente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_global_root(id: i64, handle: i64) {
    HEAP.with(|heap| heap.borrow_mut().set_global_root(id, handle));
}
/// Marca um valor canônico (constante, valor de enum, global `const`) como
/// permanente e imutável: uma mensagem no mesmo isolado o passa pela
/// identidade (`ports.rs`). O valor já é raiz global de quem chama.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_marcar_permanente(handle: i64) {
    HEAP.with(|heap| heap.borrow_mut().mark_permanent(handle));
}
/// Marca a constante canônica `handle`, produzida pelo getter gerado no
/// endereço `getter` (ver `Heap::mark_constant`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_marcar_constante(handle: i64, getter: i64) {
    HEAP.with(|heap| heap.borrow_mut().mark_constant(handle, getter as usize));
}
/// A forma canônica do valor de uma constante, antes de o getter guardá-lo: a
/// string montada (`const s = 'a${'b'}'`) vira o literal canônico de mesmo
/// conteúdo, como a canonicalização de constantes da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_constante_canonica(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().canonical_constant(handle))
}
/// Permite coleta explícita em testes e futuras rotinas de manutenção.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_collect() {
    HEAP.with(|heap| heap.borrow_mut().collect());
}

// ---------------------------------------------------------------------------
// A área de globais do isolado.
//
// Os estáticos do Dart (e os caches dos pontos de chamada por seletor) são
// por isolado, como a *field table* da VM: cada módulo compilado descreve
// os slots dele (`@df.area`: `[key, n, nome_0…nome_{n-1}]`, com hashes;
// no executável de produção, que não recarrega, só `[key, -n]`) e
// cada isolado — uma thread — tem uma área zerada por módulo, criada no
// primeiro acesso. Os globais `Ref` são raízes pelo endereço do slot
// (`dartforge_gc_global_root`).
//
// Numa recarga (JIT), o módulo novo tem outro descritor com a mesma chave.
// A geração nova é emitida com o layout da viva (`emitir_ir_recarregavel`):
// os slots que continuam mantêm o índice e os novos vêm depois, então as
// duas gerações usam a MESMA área — que cresce no lugar (a capacidade é
// reservada com folga) — e o código antigo que ainda executa (um quadro
// `async` suspenso, uma closure criada antes) vê os mesmos estáticos que o
// novo. Um descritor que não estende o layout (um módulo emitido sem o
// layout anterior) recebe uma área nova, com os valores dos slots que
// continuam, pelo nome; os caches de seletor (nome com `BIT_DE_CACHE`, um
// par por ponto de chamada, nomeado pela função e pela posição) recomeçam
// zerados, e toda publicação os zera (`empty_area_caches`).
//
// Memória de uma área nunca é liberada enquanto o isolado vive: um quadro
// pode guardar o endereço dela (o `%area` do começo da função). Crescer
// além da capacidade copia para outra alocação e aposenta a antiga.

/// A área de um módulo neste isolado.
struct GlobalsArea {
    /// Os descritores (as gerações do módulo) que usam esta área.
    descriptors: Vec<usize>,
    key: i64,
    /// Os nomes do layout mais longo visto (o da geração mais nova).
    names: Vec<i64>,
    slots: Vec<i64>,
}

thread_local! {
    static AREAS: RefCell<Vec<GlobalsArea>> = const { RefCell::new(Vec::new()) };
    /// As alocações que deixaram de ser a de uma área (ver acima).
    static RETIRED_AREAS: RefCell<Vec<Vec<i64>>> = const { RefCell::new(Vec::new()) };
    /// O último descritor pedido e a área dele (o caminho rápido).
    static ULTIMA_AREA: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    /// A tabela por id de módulo que o `Context` aponta (`areas`,
    /// `n_areas`).
    static AREA_TABLE: RefCell<Vec<*mut i64>> = const { RefCell::new(Vec::new()) };
}

/// O próximo id de módulo (`@df.area_id`), o mesmo em todas as threads.
static PROXIMO_ID_DE_AREA: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);

/// Uma área mudou de endereço (recarga): a tabela desta thread se esvazia
/// e cada módulo passa uma vez pelo runtime de novo.
fn forget_area_table() {
    AREA_TABLE.with(|t| t.borrow_mut().iter_mut().for_each(|p| *p = std::ptr::null_mut()));
    ULTIMA_AREA.with(|u| u.set((0, 0)));
}

/// Depois da publicação de uma geração nova neste isolado (sem quadro Dart
/// na pilha, J02): cada área fica só com o descritor da geração mais nova
/// (o último que chegou) — o de uma geração aposentada é um endereço da
/// memória dela, que o JIT pode liberar e reaproveitar — e as alocações
/// aposentadas, que só um `%area` de um quadro antigo ainda usaria, são
/// soltas.
pub fn forget_previous_area_generations() {
    AREAS.with(|areas| {
        for a in areas.borrow_mut().iter_mut() {
            if let Some(&new) = a.descriptors.last() {
                a.descriptors.clear();
                a.descriptors.push(new);
            }
        }
    });
    RETIRED_AREAS.with(|x| x.borrow_mut().clear());
    forget_area_table();
}

/// O caminho lento de `@df.obter_area`: a área (`dartforge_area_de_globais`)
/// e, na tabela desta thread, a entrada do id do módulo (dado agora, se
/// ainda não tem).
///
/// # Safety
/// `descriptor` como em [`dartforge_area_de_globais`]; `id` aponta o
/// `@df.area_id` do mesmo módulo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_area_de_globais_id(descriptor: *const i64, id: *const std::sync::atomic::AtomicI64) -> *mut i64 {
    use std::sync::atomic::Ordering;
    // SAFETY: garantido por quem chama.
    let p = unsafe { dartforge_area_de_globais(descriptor) };
    // SAFETY: o global do módulo, vivo durante todo o processo.
    let id = unsafe { &*id };
    let mut k = id.load(Ordering::Relaxed);
    if k == 0 {
        let new = PROXIMO_ID_DE_AREA.fetch_add(1, Ordering::Relaxed);
        k = match id.compare_exchange(0, new, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => new,
            Err(current) => current,
        };
    }
    let k = k as usize;
    AREA_TABLE.with(|t| {
        let mut t = t.borrow_mut();
        if t.len() <= k {
            t.resize(k + 16, std::ptr::null_mut());
        }
        t[k] = p;
        CONTEXTO.with(|c| {
            c.areas.set(t.as_ptr());
            c.n_areas.set(t.len());
        });
    });
    p
}

/// Slots zerados para `n` nomes, com folga para as gerações seguintes
/// crescerem no lugar.
fn new_slots(n: usize) -> Vec<i64> {
    let mut v = Vec::with_capacity(n + n / 2 + 256);
    v.resize(n, 0);
    v
}

/// A área de globais do módulo de `descriptor` neste isolado.
///
/// # Safety
/// `descriptor` aponta para o `@df.area` de um módulo: `n + 2` palavras
/// (`[key, n, names…]`), ou 2 (`[key, -n]`, o enxuto de produção),
/// constantes durante todo o processo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_area_de_globais(descriptor: *const i64) -> *mut i64 {
    let d = descriptor as usize;
    let (last_one, area) = ULTIMA_AREA.with(|u| u.get());
    if last_one == d {
        return area as *mut i64;
    }
    // SAFETY: garantido por quem chama. O descritor enxuto do executável de
    // produção (`[key, -n]`, sem os nomes) não recarrega: n slots zerados,
    // sem nomes e sem folga.
    let (key, names, lean) = unsafe {
        let raw = *descriptor.add(1);
        if raw < 0 {
            (*descriptor, &[][..], Some(raw.unsigned_abs() as usize))
        } else {
            (*descriptor, std::slice::from_raw_parts(descriptor.add(2), raw as usize), None)
        }
    };
    let p = AREAS.with(|areas| {
        let mut areas = areas.borrow_mut();
        if let Some(a) = areas.iter_mut().find(|a| a.descriptors.contains(&d)) {
            return a.slots.as_mut_ptr();
        }
        if let Some(n) = lean {
            areas.push(GlobalsArea { descriptors: vec![d], key, names: Vec::new(), slots: vec![0; n] });
            return areas.last_mut().expect("acabou de entrar").slots.as_mut_ptr();
        }
        if let Some(i) = areas.iter().position(|a| a.key == key) {
            let a = &mut areas[i];
            if names.len() >= a.names.len() && names[..a.names.len()] == a.names[..] {
                extend_area(a, names);
                a.descriptors.push(d);
                return a.slots.as_mut_ptr();
            }
            // O descritor de uma geração anterior (o layout só cresce): o
            // código dela que continua em uso — uma função que a recarga
            // manteve (J04) — só conhece slots que existem com o mesmo nome
            // no mesmo índice.
            if names.len() < a.names.len() && a.names[..names.len()] == *names {
                a.descriptors.push(d);
                return a.slots.as_mut_ptr();
            }
            let old_value = areas.remove(i);
            let mut new_area = GlobalsArea { descriptors: vec![d], key, names: names.to_vec(), slots: new_slots(names.len()) };
            migrate_area(&old_value, &mut new_area);
            RETIRED_AREAS.with(|x| x.borrow_mut().push(old_value.slots));
            areas.push(new_area);
            forget_area_table();
        } else {
            areas.push(GlobalsArea { descriptors: vec![d], key, names: names.to_vec(), slots: new_slots(names.len()) });
        }
        areas.last_mut().expect("acabou de entrar").slots.as_mut_ptr()
    });
    ULTIMA_AREA.with(|u| u.set((d, p as usize)));
    p
}

/// Estende `a` ao layout `names` (que começa pelo dela): no lugar, se cabe;
/// senão numa alocação maior, com as raízes movidas e a antiga aposentada.
fn extend_area(a: &mut GlobalsArea, names: &[i64]) {
    if names.len() > a.slots.capacity() {
        let mut larger = new_slots(names.len());
        larger[..a.slots.len()].copy_from_slice(&a.slots);
        let address = |slots: &[i64], i: usize| (&slots[i] as *const i64) as i64;
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            for i in 0..a.slots.len() {
                heap.move_global_root(address(&a.slots, i), address(&larger, i));
            }
        });
        let old_value = std::mem::replace(&mut a.slots, larger);
        RETIRED_AREAS.with(|x| x.borrow_mut().push(old_value));
        forget_area_table();
    } else {
        a.slots.resize(names.len(), 0);
    }
    a.names = names.to_vec();
}

/// O bit que marca, no descritor, o nome de um slot de cache de seletor
/// (`BIT_DE_CACHE` do emissor, `llvm/mod.rs`); 0 é o nome de cache de um
/// descritor anterior a esta marca.
const BIT_DE_CACHE: i64 = 1 << 62;

fn is_cache(name: i64) -> bool {
    name == 0 || name & BIT_DE_CACHE != 0
}

/// Zera os caches de seletor de todas as áreas deste isolado. A publicação
/// de uma geração o faz sem quadro Dart na pilha, depois de refazer as
/// tabelas de métodos: o próximo uso de cada cache busca a entrada na tabela
/// nova (a regra de visibilidade: chamadas novas, também do código de uma
/// geração anterior, chegam à implementação nova). Um cache guarda id de
/// classe e endereço de código, nunca referência do heap.
pub fn empty_area_caches() {
    AREAS.with(|areas| {
        for a in areas.borrow_mut().iter_mut() {
            for (i, &name) in a.names.iter().enumerate() {
                if is_cache(name) && i < a.slots.len() {
                    a.slots[i] = 0;
                }
            }
        }
    });
}

/// Copia para `new_area` os slots de `old_value` com o mesmo nome e move as raízes
/// deles; as dos slots que sumiram são soltas. Os caches não migram.
fn migrate_area(old_value: &GlobalsArea, new_area: &mut GlobalsArea) {
    let address = |slots: &[i64], i: usize| (&slots[i] as *const i64) as i64;
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        for (i, &name) in old_value.names.iter().enumerate() {
            let dest = if is_cache(name) { None } else { new_area.names.iter().position(|&n| n == name) };
            match dest {
                Some(j) => {
                    new_area.slots[j] = old_value.slots[i];
                    heap.move_global_root(address(&old_value.slots, i), address(&new_area.slots, j));
                }
                None => heap.release_global_root(address(&old_value.slots, i)),
            }
        }
    });
}

/// Raízes por mapas de pilha (`--roots=maps`): o módulo com funções
/// descritas por mapa registra, na partida, a imagem em que foi ligado
/// (`crate::heap::register_stack_map`; idempotente por imagem). Não
/// aloca no heap do coletor nem lança.
///
/// # Safety
/// `base` é o `__ImageBase` de uma imagem PE carregada.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_mapa(base: *const u8) {
    crate::heap::register_stack_map(base as usize);
}

/// O registro do mapa de pilha compacto de uma imagem ELF ou Mach-O
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md, Etapa 4): o módulo com funções
/// de raízes no mapa chama isto na partida com os limites da seção `dfgcm`
/// que o ligador definiu. Idempotente por imagem.
///
/// # Safety
/// `[start, end)` é a seção do mapa de uma imagem carregada.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_mapa_secao(start: *const u8, end: *const u8) {
    crate::heap::register_map_section(start as usize, end as usize);
}

/// O registro do mapa de pilha no formato do LLVM de uma imagem Mach-O
/// (Etapa 4): os limites da seção `__LLVM_STACKMAPS,__llvm_stackmaps`.
///
/// # Safety
/// `[start, end)` é a seção do mapa de uma imagem carregada.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_mapa_llvm(start: *const u8, end: *const u8) {
    crate::heap::register_llvm_section(start as usize, end as usize);
}

/// Tira o mapa registrado por [`dartforge_registrar_mapa_llvm`] com o mesmo
/// `start` (o JIT, ao soltar a memória de um objeto).
///
/// # Safety
/// Nenhuma: só compara o endereço.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_desregistrar_mapa(start: *const u8) {
    crate::heap::unregister_map(start as usize);
}

/// As estatísticas que um teste dirigido lê no fim (§7.5, a prova de que
/// coletou): grava em `out[0..4]` as coletas do isolado corrente, os
/// quadros nativos percorridos, as raízes lidas de mapas e as raízes
/// conferidas contra a pilha-sombra (as três últimas, do processo).
///
/// # Safety
/// `out` aponta para quatro `u64` graváveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_gc_estatisticas(out: *mut u64) {
    if out.is_null() {
        return;
    }
    let gc_count = HEAP.with(|h| h.borrow().stats().collections);
    let (frames, roots, checked) = crate::heap::walk_counts();
    // SAFETY: o chamador garante os quatro `u64`.
    unsafe {
        *out = gc_count;
        *out.add(1) = frames;
        *out.add(2) = roots;
        *out.add(3) = checked;
    }
}

#[cfg(test)]
mod tests_gc_roots {
    use super::*;

    #[test]
    fn lean_descriptor_gives_n_zeroed_slots_and_same_area() {
        std::thread::spawn(|| {
            // `[key, -n]` (produção) e `[key, n, names…]` (recarga), de
            // módulos diferentes, no mesmo isolado.
            static LEAN: [i64; 2] = [0x1234, -3];
            static COMPLETE: [i64; 4] = [0x5678, 2, 11, 12];
            // SAFETY: descritores constantes do processo inteiro.
            let (a, b, c) = unsafe {
                (
                    dartforge_area_de_globais(LEAN.as_ptr()),
                    dartforge_area_de_globais(COMPLETE.as_ptr()),
                    dartforge_area_de_globais(LEAN.as_ptr()),
                )
            };
            assert_eq!(a, c);
            assert_ne!(a, b);
            // SAFETY: a área tem os 3 slots do descritor.
            let slots = unsafe { std::slice::from_raw_parts_mut(a, 3) };
            assert_eq!(slots, [0, 0, 0]);
            slots[2] = 7;
            AREAS.with(|areas| {
                let areas = areas.borrow();
                let e = areas.iter().find(|x| x.key == 0x1234).expect("área enxuta");
                assert!(e.names.is_empty());
                assert_eq!(e.slots, [0, 0, 7]);
                let c = areas.iter().find(|x| x.key == 0x5678).expect("área completa");
                assert_eq!(c.names, [11, 12]);
            });
        })
        .join()
        .unwrap();
    }
}

// ─── Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ───────────────

/// A alocação lenta do código gerado (`@df.allocate`): um bloco zerado de
/// `words` palavras de corpo, classe `cid`, estado `JOVEM`; coleta se preciso
/// e reabastece a TLAB de `words` (até `layout::TLAB_N`). Não lança.
///
/// `flags` são os bits 8–31 da palavra 0 do cabeçalho que o código gerado
/// montou (`header >> 8`): os `flags` do bloco nos 8 de baixo e, em
/// `INSTANCIA`, o número de campos nos 16 seguintes (em `BRUTO`/`REFS`, o `n` é
/// `words`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_alocar(cid: i64, words: i64, flags: i64) -> i64 {
    let w = usize::try_from(words).expect("bug do compilador: palavras negativas em dartforge_alocar");
    let f = (flags & 0xFF) as u8;
    let n = if f & crate::layout::flags::FORMA == crate::layout::flags::INSTANCIA {
        ((flags >> 8) & 0xFFFF) as usize
    } else {
        w
    };
    let cid = i32::try_from(cid).expect("bug do compilador: cid além de 32 bits");
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let r = h.alocar_bloco(cid, w, f, n);
        h.refill_tlab(w);
        r
    })
}

/// Registra a seção de objetos estáticos `[start, end)` de uma imagem (o
/// executável, a DLL do SDK): chamada pelo `@df.preparar_isolado` de cada uma
/// (docs/NATIVO-ESPACO-UNIFICADO.md §2.11). Não aloca nem lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_registrar_imagem(start: *const u8, end: *const u8) {
    crate::heap::register_image(start as usize, end as usize);
}
