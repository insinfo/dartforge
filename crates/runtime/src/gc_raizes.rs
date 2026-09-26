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

/// Abre frame para raízes precisas dos valores SSA da função.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_push_frame(slot_count: i64) -> i64 {
    HEAP.with(|heap| {
        heap.borrow_mut().push_frame_with_slots(
            usize::try_from(slot_count).expect("quantidade de slots inválida"),
        )
    })
}
/// Substitui uma raiz estática; zero limpa o slot sem alterar o tamanho do frame.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_set_root(frame: i64, slot: i64, handle: i64) {
    HEAP.with(|heap| {
        heap.borrow_mut()
            .set_root(frame, usize::try_from(slot).expect("slot inválido"), handle)
    });
}
/// Protege handle positivo; zero representa null.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_root(frame: i64, handle: i64) {
    HEAP.with(|heap| heap.borrow_mut().root(frame, handle));
}
/// Remove raízes do frame sem disparar coleta durante retorno ao chamador.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_pop_frame(frame: i64) {
    HEAP.with(|heap| heap.borrow_mut().pop_frame(frame));
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
// Numa recarga (JIT), o módulo novo tem outro descritor com a mesma chave:
// a área nova recebe, pelo nome, os valores dos slots que continuam
// existindo, e as raízes mudam de endereço junto; os slots de nome 0 (os
// caches de seletor, que guardam endereços do código antigo) recomeçam
// zerados.

/// A área de um módulo neste isolado.
struct AreaDeGlobais {
    descritor: usize,
    chave: i64,
    slots: Box<[i64]>,
}

thread_local! {
    static AREAS: RefCell<Vec<AreaDeGlobais>> = const { RefCell::new(Vec::new()) };
    /// O último descritor pedido e a área dele (o caminho rápido).
    static ULTIMA_AREA: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
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
        if let Some(a) = areas.iter_mut().find(|a| a.descritor == d) {
            return a.slots.as_mut_ptr();
        }
        let mut nova = AreaDeGlobais { descritor: d, chave, slots: vec![0i64; nomes.len()].into_boxed_slice() };
        if let Some(i) = areas.iter().position(|a| a.chave == chave) {
            let antiga = areas.remove(i);
            migrar_area(&antiga, &mut nova, nomes);
        }
        areas.push(nova);
        areas.last_mut().expect("acabou de entrar").slots.as_mut_ptr()
    });
    ULTIMA_AREA.with(|u| u.set((d, p as usize)));
    p
}

/// Copia para `nova` os slots de `antiga` com o mesmo nome e move as raízes
/// deles; as dos slots que sumiram são soltas.
fn migrar_area(antiga: &AreaDeGlobais, nova: &mut AreaDeGlobais, nomes_novos: &[i64]) {
    // SAFETY: o descritor antigo é uma constante do módulo antigo, que o JIT
    // mantém carregado enquanto houver área dele.
    let nomes_antigos = unsafe {
        let p = antiga.descritor as *const i64;
        std::slice::from_raw_parts(p.add(2), *p.add(1) as usize)
    };
    let endereco = |slots: &[i64], i: usize| (&slots[i] as *const i64) as i64;
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        for (i, &nome) in nomes_antigos.iter().enumerate() {
            let destino = if nome == 0 { None } else { nomes_novos.iter().position(|&n| n == nome) };
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
