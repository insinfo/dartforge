// Runtime nativo: protocolo de raízes do coletor (frames, globais, coleta).

/// Executa `f` com `handles` enraizados num frame temporário (G6).
///
/// Extern que aloca mais de uma vez: o primeiro objeto só é referenciado
/// por uma variável do Rust enquanto o segundo é alocado, e essa alocação
/// pode coletar. O frame é o mesmo protocolo do código gerado.
fn com_raizes<R>(handles: &[i64], f: impl FnOnce() -> R) -> R {
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

/// O endereço do contexto da thread corrente (`heap::Contexto`): a exceção
/// pendente e o topo da pilha-sombra, que o código gerado lê e grava sem
/// chamada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_contexto() -> *const crate::heap::Contexto {
    crate::heap::CONTEXTO.with(|c| {
        if c.limite_da_pilha.get() == 0 {
            c.limite_da_pilha.set(limite_da_pilha_da_thread());
        }
        c as *const crate::heap::Contexto
    })
}

/// Folga entre o limite que o código gerado confere e o fim da pilha da
/// thread: o que o runtime ainda usa para montar e lançar o
/// `StackOverflowError` (o rastro, a alocação) e para as chamadas nativas
/// de uma função que passou do prólogo.
const FOLGA_DA_PILHA: usize = 256 * 1024;

/// O endereço mais baixo que a pilha da thread corrente pode usar, mais a
/// [`FOLGA_DA_PILHA`]; 1 quando o sistema não diz (sem conferência).
#[allow(unsafe_code)]
fn limite_da_pilha_da_thread() -> usize {
    let base = base_da_pilha();
    // A sabotagem `folga` (D8, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    // §7.3): 4 KiB não bastam para montar e lançar o `StackOverflowError`.
    let folga = if crate::heap::sabotagem("folga") { 4 * 1024 } else { FOLGA_DA_PILHA };
    if base == 0 { 1 } else { base + folga }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn base_da_pilha() -> usize {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadStackLimits(baixo: *mut usize, alto: *mut usize);
    }
    let (mut baixo, mut alto) = (0usize, 0usize);
    // SAFETY: dois ponteiros para `usize` locais.
    unsafe { GetCurrentThreadStackLimits(&mut baixo, &mut alto) };
    baixo
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn base_da_pilha() -> usize {
    // `pthread_attr_t` tem 56 bytes no glibc x86-64 e 64 no aarch64.
    #[repr(C, align(8))]
    struct Atributos([u8; 64]);
    unsafe extern "C" {
        fn pthread_self() -> usize;
        fn pthread_getattr_np(t: usize, a: *mut Atributos) -> i32;
        fn pthread_attr_getstack(a: *const Atributos, endereco: *mut usize, tamanho: *mut usize) -> i32;
        fn pthread_attr_destroy(a: *mut Atributos) -> i32;
    }
    let mut a = Atributos([0; 64]);
    let (mut endereco, mut tamanho) = (0usize, 0usize);
    // SAFETY: atributos locais, iniciados pelo `pthread_getattr_np` e
    // destruídos no fim.
    unsafe {
        if pthread_getattr_np(pthread_self(), &mut a) != 0 {
            return 0;
        }
        let ok = pthread_attr_getstack(&a, &mut endereco, &mut tamanho) == 0;
        pthread_attr_destroy(&mut a);
        if ok { endereco } else { 0 }
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn base_da_pilha() -> usize {
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
fn base_da_pilha() -> usize {
    0
}

/// O prólogo de uma função achou o quadro abaixo do limite da pilha
/// (`Contexto::limite_da_pilha`): lança o `StackOverflowError` do SDK, e a
/// função volta com a exceção pendente, como depois de uma chamada que
/// lançou. O limite é desligado enquanto o erro é montado (o rastro e a
/// alocação usam a folga) e religado depois.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_estouro_de_pilha() {
    let limite = crate::heap::CONTEXTO.with(|c| c.limite_da_pilha.replace(1));
    let erro = dartforge_stack_overflow_error_new();
    if dartforge_exception_pending() == 0 {
        com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
    }
    crate::heap::CONTEXTO.with(|c| c.limite_da_pilha.set(limite));
}

/// Valor corrente de um global `Ref` do programa, mantido como raiz permanente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_global_root(id: i64, handle: i64) {
    HEAP.with(|heap| heap.borrow_mut().set_global_root(id, handle));
}
/// Marca um valor canônico (constante, valor de enum, global `const`) como
/// permanente e imutável: uma mensagem no mesmo isolado o passa pela
/// identidade (`portas.rs`). O valor já é raiz global de quem chama.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_marcar_permanente(handle: i64) {
    HEAP.with(|heap| heap.borrow_mut().marcar_permanente(handle));
}
/// Marca a constante canônica `handle`, produzida pelo getter gerado no
/// endereço `getter` (ver `Heap::marcar_constante`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_marcar_constante(handle: i64, getter: i64) {
    HEAP.with(|heap| heap.borrow_mut().marcar_constante(handle, getter as usize));
}
/// A forma canônica do valor de uma constante, antes de o getter guardá-lo: a
/// string montada (`const s = 'a${'b'}'`) vira o literal canônico de mesmo
/// conteúdo, como a canonicalização de constantes da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_constante_canonica(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().constante_canonica(handle))
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
// os slots dele (`@df.area`: `[chave, n, nome_0…nome_{n-1}]`, com hashes;
// no executável de produção, que não recarrega, só `[chave, -n]`) e
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
// zerados, e toda publicação os zera (`esvaziar_caches_das_areas`).
//
// Memória de uma área nunca é liberada enquanto o isolado vive: um quadro
// pode guardar o endereço dela (o `%area` do começo da função). Crescer
// além da capacidade copia para outra alocação e aposenta a antiga.

/// A área de um módulo neste isolado.
struct AreaDeGlobais {
    /// Os descritores (as gerações do módulo) que usam esta área.
    descritores: Vec<usize>,
    chave: i64,
    /// Os nomes do layout mais longo visto (o da geração mais nova).
    nomes: Vec<i64>,
    slots: Vec<i64>,
}

thread_local! {
    static AREAS: RefCell<Vec<AreaDeGlobais>> = const { RefCell::new(Vec::new()) };
    /// As alocações que deixaram de ser a de uma área (ver acima).
    static AREAS_APOSENTADAS: RefCell<Vec<Vec<i64>>> = const { RefCell::new(Vec::new()) };
    /// O último descritor pedido e a área dele (o caminho rápido).
    static ULTIMA_AREA: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    /// A tabela por id de módulo que o `Contexto` aponta (`areas`,
    /// `n_areas`).
    static TABELA_DE_AREAS: RefCell<Vec<*mut i64>> = const { RefCell::new(Vec::new()) };
}

/// O próximo id de módulo (`@df.area_id`), o mesmo em todas as threads.
static PROXIMO_ID_DE_AREA: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);

/// Uma área mudou de endereço (recarga): a tabela desta thread se esvazia
/// e cada módulo passa uma vez pelo runtime de novo.
fn esquecer_tabela_de_areas() {
    TABELA_DE_AREAS.with(|t| t.borrow_mut().iter_mut().for_each(|p| *p = std::ptr::null_mut()));
    ULTIMA_AREA.with(|u| u.set((0, 0)));
}

/// Depois da publicação de uma geração nova neste isolado (sem quadro Dart
/// na pilha, J02): cada área fica só com o descritor da geração mais nova
/// (o último que chegou) — o de uma geração aposentada é um endereço da
/// memória dela, que o JIT pode liberar e reaproveitar — e as alocações
/// aposentadas, que só um `%area` de um quadro antigo ainda usaria, são
/// soltas.
pub fn esquecer_geracoes_anteriores_das_areas() {
    AREAS.with(|areas| {
        for a in areas.borrow_mut().iter_mut() {
            if let Some(&novo) = a.descritores.last() {
                a.descritores.clear();
                a.descritores.push(novo);
            }
        }
    });
    AREAS_APOSENTADAS.with(|x| x.borrow_mut().clear());
    esquecer_tabela_de_areas();
}

/// O caminho lento de `@df.obter_area`: a área (`dartforge_area_de_globais`)
/// e, na tabela desta thread, a entrada do id do módulo (dado agora, se
/// ainda não tem).
///
/// # Safety
/// `descritor` como em [`dartforge_area_de_globais`]; `id` aponta o
/// `@df.area_id` do mesmo módulo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_area_de_globais_id(descritor: *const i64, id: *const std::sync::atomic::AtomicI64) -> *mut i64 {
    use std::sync::atomic::Ordering;
    // SAFETY: garantido por quem chama.
    let p = unsafe { dartforge_area_de_globais(descritor) };
    // SAFETY: o global do módulo, vivo durante todo o processo.
    let id = unsafe { &*id };
    let mut k = id.load(Ordering::Relaxed);
    if k == 0 {
        let novo = PROXIMO_ID_DE_AREA.fetch_add(1, Ordering::Relaxed);
        k = match id.compare_exchange(0, novo, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => novo,
            Err(atual) => atual,
        };
    }
    let k = k as usize;
    TABELA_DE_AREAS.with(|t| {
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
fn slots_novos(n: usize) -> Vec<i64> {
    let mut v = Vec::with_capacity(n + n / 2 + 256);
    v.resize(n, 0);
    v
}

/// A área de globais do módulo de `descritor` neste isolado.
///
/// # Safety
/// `descritor` aponta para o `@df.area` de um módulo: `n + 2` palavras
/// (`[chave, n, nomes…]`), ou 2 (`[chave, -n]`, o enxuto de produção),
/// constantes durante todo o processo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_area_de_globais(descritor: *const i64) -> *mut i64 {
    let d = descritor as usize;
    let (ultimo, area) = ULTIMA_AREA.with(|u| u.get());
    if ultimo == d {
        return area as *mut i64;
    }
    // SAFETY: garantido por quem chama. O descritor enxuto do executável de
    // produção (`[chave, -n]`, sem os nomes) não recarrega: n slots zerados,
    // sem nomes e sem folga.
    let (chave, nomes, enxuto) = unsafe {
        let bruto = *descritor.add(1);
        if bruto < 0 {
            (*descritor, &[][..], Some(bruto.unsigned_abs() as usize))
        } else {
            (*descritor, std::slice::from_raw_parts(descritor.add(2), bruto as usize), None)
        }
    };
    let p = AREAS.with(|areas| {
        let mut areas = areas.borrow_mut();
        if let Some(a) = areas.iter_mut().find(|a| a.descritores.contains(&d)) {
            return a.slots.as_mut_ptr();
        }
        if let Some(n) = enxuto {
            areas.push(AreaDeGlobais { descritores: vec![d], chave, nomes: Vec::new(), slots: vec![0; n] });
            return areas.last_mut().expect("acabou de entrar").slots.as_mut_ptr();
        }
        if let Some(i) = areas.iter().position(|a| a.chave == chave) {
            let a = &mut areas[i];
            if nomes.len() >= a.nomes.len() && nomes[..a.nomes.len()] == a.nomes[..] {
                estender_area(a, nomes);
                a.descritores.push(d);
                return a.slots.as_mut_ptr();
            }
            // O descritor de uma geração anterior (o layout só cresce): o
            // código dela que continua em uso — uma função que a recarga
            // manteve (J04) — só conhece slots que existem com o mesmo nome
            // no mesmo índice.
            if nomes.len() < a.nomes.len() && a.nomes[..nomes.len()] == *nomes {
                a.descritores.push(d);
                return a.slots.as_mut_ptr();
            }
            let antiga = areas.remove(i);
            let mut nova = AreaDeGlobais { descritores: vec![d], chave, nomes: nomes.to_vec(), slots: slots_novos(nomes.len()) };
            migrar_area(&antiga, &mut nova);
            AREAS_APOSENTADAS.with(|x| x.borrow_mut().push(antiga.slots));
            areas.push(nova);
            esquecer_tabela_de_areas();
        } else {
            areas.push(AreaDeGlobais { descritores: vec![d], chave, nomes: nomes.to_vec(), slots: slots_novos(nomes.len()) });
        }
        areas.last_mut().expect("acabou de entrar").slots.as_mut_ptr()
    });
    ULTIMA_AREA.with(|u| u.set((d, p as usize)));
    p
}

/// Estende `a` ao layout `nomes` (que começa pelo dela): no lugar, se cabe;
/// senão numa alocação maior, com as raízes movidas e a antiga aposentada.
fn estender_area(a: &mut AreaDeGlobais, nomes: &[i64]) {
    if nomes.len() > a.slots.capacity() {
        let mut maior = slots_novos(nomes.len());
        maior[..a.slots.len()].copy_from_slice(&a.slots);
        let endereco = |slots: &[i64], i: usize| (&slots[i] as *const i64) as i64;
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            for i in 0..a.slots.len() {
                heap.mover_raiz_global(endereco(&a.slots, i), endereco(&maior, i));
            }
        });
        let antiga = std::mem::replace(&mut a.slots, maior);
        AREAS_APOSENTADAS.with(|x| x.borrow_mut().push(antiga));
        esquecer_tabela_de_areas();
    } else {
        a.slots.resize(nomes.len(), 0);
    }
    a.nomes = nomes.to_vec();
}

/// O bit que marca, no descritor, o nome de um slot de cache de seletor
/// (`BIT_DE_CACHE` do emissor, `llvm/mod.rs`); 0 é o nome de cache de um
/// descritor anterior a esta marca.
const BIT_DE_CACHE: i64 = 1 << 62;

fn e_cache(nome: i64) -> bool {
    nome == 0 || nome & BIT_DE_CACHE != 0
}

/// Zera os caches de seletor de todas as áreas deste isolado. A publicação
/// de uma geração o faz sem quadro Dart na pilha, depois de refazer as
/// tabelas de métodos: o próximo uso de cada cache busca a entrada na tabela
/// nova (a regra de visibilidade: chamadas novas, também do código de uma
/// geração anterior, chegam à implementação nova). Um cache guarda id de
/// classe e endereço de código, nunca referência do heap.
pub fn esvaziar_caches_das_areas() {
    AREAS.with(|areas| {
        for a in areas.borrow_mut().iter_mut() {
            for (i, &nome) in a.nomes.iter().enumerate() {
                if e_cache(nome) && i < a.slots.len() {
                    a.slots[i] = 0;
                }
            }
        }
    });
}

/// Copia para `nova` os slots de `antiga` com o mesmo nome e move as raízes
/// deles; as dos slots que sumiram são soltas. Os caches não migram.
fn migrar_area(antiga: &AreaDeGlobais, nova: &mut AreaDeGlobais) {
    let endereco = |slots: &[i64], i: usize| (&slots[i] as *const i64) as i64;
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        for (i, &nome) in antiga.nomes.iter().enumerate() {
            let destino = if e_cache(nome) { None } else { nova.nomes.iter().position(|&n| n == nome) };
            match destino {
                Some(j) => {
                    nova.slots[j] = antiga.slots[i];
                    heap.mover_raiz_global(endereco(&antiga.slots, i), endereco(&nova.slots, j));
                }
                None => heap.soltar_raiz_global(endereco(&antiga.slots, i)),
            }
        }
    });
}

/// Raízes por mapas de pilha (`--raizes=mapas`): o módulo com funções
/// descritas por mapa registra, na partida, a imagem em que foi ligado
/// (`crate::heap::registrar_mapa_de_pilha`; idempotente por imagem). Não
/// aloca no heap do coletor nem lança.
///
/// # Safety
/// `base` é o `__ImageBase` de uma imagem PE carregada.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_mapa(base: *const u8) {
    crate::heap::registrar_mapa_de_pilha(base as usize);
}

/// O registro do mapa de pilha compacto de uma imagem ELF ou Mach-O
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md, Etapa 4): o módulo com funções
/// de raízes no mapa chama isto na partida com os limites da seção `dfgcm`
/// que o ligador definiu. Idempotente por imagem.
///
/// # Safety
/// `[inicio, fim)` é a seção do mapa de uma imagem carregada.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_mapa_secao(inicio: *const u8, fim: *const u8) {
    crate::heap::registrar_secao_de_mapa(inicio as usize, fim as usize);
}

/// O registro do mapa de pilha no formato do LLVM de uma imagem Mach-O
/// (Etapa 4): os limites da seção `__LLVM_STACKMAPS,__llvm_stackmaps`.
///
/// # Safety
/// `[inicio, fim)` é a seção do mapa de uma imagem carregada.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_mapa_llvm(inicio: *const u8, fim: *const u8) {
    crate::heap::registrar_secao_llvm(inicio as usize, fim as usize);
}

/// Tira o mapa registrado por [`dartforge_registrar_mapa_llvm`] com o mesmo
/// `inicio` (o JIT, ao soltar a memória de um objeto).
///
/// # Safety
/// Nenhuma: só compara o endereço.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_desregistrar_mapa(inicio: *const u8) {
    crate::heap::desregistrar_mapa(inicio as usize);
}

/// As estatísticas que um teste dirigido lê no fim (§7.5, a prova de que
/// coletou): grava em `saida[0..4]` as coletas do isolado corrente, os
/// quadros nativos percorridos, as raízes lidas de mapas e as raízes
/// conferidas contra a pilha-sombra (as três últimas, do processo).
///
/// # Safety
/// `saida` aponta para quatro `u64` graváveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_gc_estatisticas(saida: *mut u64) {
    if saida.is_null() {
        return;
    }
    let coletas = HEAP.with(|h| h.borrow().stats().collections);
    let (quadros, raizes, conferidas) = crate::heap::numeros_do_percurso();
    // SAFETY: o chamador garante os quatro `u64`.
    unsafe {
        *saida = coletas;
        *saida.add(1) = quadros;
        *saida.add(2) = raizes;
        *saida.add(3) = conferidas;
    }
}

#[cfg(test)]
mod testes_gc_raizes {
    use super::*;

    #[test]
    fn descritor_enxuto_da_n_slots_zerados_e_a_mesma_area() {
        std::thread::spawn(|| {
            // `[chave, -n]` (produção) e `[chave, n, nomes…]` (recarga), de
            // módulos diferentes, no mesmo isolado.
            static ENXUTO: [i64; 2] = [0x1234, -3];
            static COMPLETO: [i64; 4] = [0x5678, 2, 11, 12];
            // SAFETY: descritores constantes do processo inteiro.
            let (a, b, c) = unsafe {
                (
                    dartforge_area_de_globais(ENXUTO.as_ptr()),
                    dartforge_area_de_globais(COMPLETO.as_ptr()),
                    dartforge_area_de_globais(ENXUTO.as_ptr()),
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
                let e = areas.iter().find(|x| x.chave == 0x1234).expect("área enxuta");
                assert!(e.nomes.is_empty());
                assert_eq!(e.slots, [0, 0, 7]);
                let c = areas.iter().find(|x| x.chave == 0x5678).expect("área completa");
                assert_eq!(c.nomes, [11, 12]);
            });
        })
        .join()
        .unwrap();
    }
}

// ─── Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ───────────────

/// A alocação lenta do código gerado (`@df.alocar`): um bloco zerado de
/// `palavras` palavras de corpo, classe `cid`, estado `JOVEM`; coleta se preciso
/// e reabastece a TLAB de `palavras` (até `layout::TLAB_N`). Não lança.
///
/// `flags` são os bits 8–31 da palavra 0 do cabeçalho que o código gerado
/// montou (`cabecalho >> 8`): os `flags` do bloco nos 8 de baixo e, em
/// `INSTANCIA`, o número de campos nos 16 seguintes (em `BRUTO`/`REFS`, o `n` é
/// `palavras`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_alocar(cid: i64, palavras: i64, flags: i64) -> i64 {
    let w = usize::try_from(palavras).expect("bug do compilador: palavras negativas em dartforge_alocar");
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
        h.reabastecer_tlab(w);
        r
    })
}

/// Registra a seção de objetos estáticos `[inicio, fim)` de uma imagem (o
/// executável, a DLL do SDK): chamada pelo `@df.preparar_isolado` de cada uma
/// (docs/NATIVO-ESPACO-UNIFICADO.md §2.11). Não aloca nem lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_registrar_imagem(inicio: *const u8, fim: *const u8) {
    crate::heap::registrar_imagem(inicio as usize, fim as usize);
}
