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
    crate::heap::CONTEXTO.with(|c| c as *const crate::heap::Contexto)
}

/// Encadeia o quadro de raízes de uma função gerada, no stack dela
/// (`crate::heap::QuadroDeRaizes`): o prólogo escreve o número de slots e
/// os zera; cada raiz depois é um `store` no slot.
///
/// # Safety
/// `quadro` é o quadro no stack da função gerada que chama.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_gc_empilhar(quadro: *mut crate::heap::QuadroDeRaizes) {
    // SAFETY: o contrato acima, que o emissor cumpre.
    unsafe { crate::heap::empilhar_quadro(quadro) };
}

/// Desencadeia o quadro de raízes antes de cada retorno da função.
///
/// # Safety
/// `quadro` é o topo, empilhado pela mesma função.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_gc_desempilhar(quadro: *const crate::heap::QuadroDeRaizes) {
    // SAFETY: o contrato acima, que o emissor cumpre.
    unsafe { crate::heap::desempilhar_quadro(quadro) };
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
// os slots dele (`@df.area`: `[chave, n, nome_0…nome_{n-1}]`, com hashes) e
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
/// `descritor` aponta para o `@df.area` de um módulo: `n + 2` palavras,
/// constantes durante todo o processo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_area_de_globais(descritor: *const i64) -> *mut i64 {
    let d = descritor as usize;
    let (ultimo, area) = ULTIMA_AREA.with(|u| u.get());
    if ultimo == d {
        return area as *mut i64;
    }
    // SAFETY: garantido por quem chama.
    let (chave, nomes) = unsafe {
        let n = *descritor.add(1) as usize;
        (*descritor, std::slice::from_raw_parts(descritor.add(2), n))
    };
    let p = AREAS.with(|areas| {
        let mut areas = areas.borrow_mut();
        if let Some(a) = areas.iter_mut().find(|a| a.descritores.contains(&d)) {
            return a.slots.as_mut_ptr();
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
