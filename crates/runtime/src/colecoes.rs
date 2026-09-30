// Runtime nativo: a constante canônica de coleção.
//
// Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §4.5, item 3, e o corte de
// §4.2): o legado sem SDK da fonte (`dartforge_list_*`, `_map_*`, `_set_*`,
// `_iteration_*`, `_generic_len`) saiu; fica só o que o getter de `const [...]`
// chama.

/// A constante canônica de coleção (`lower/constantes.rs`, o getter de
/// `const [...]`): uma lista vira uma `_ImmutableList` **nova** com os mesmos
/// elementos e o mesmo `E` (a classe não muda depois de publicada,
/// docs/NATIVO-ESPACO-UNIFICADO.md §2.16); quem chama usa o valor devolvido.
/// A `_ImmutableList` e o que não é lista do núcleo (o `_ConstMap`/`_ConstSet`,
/// já não modificáveis pela classe) voltam como estão.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_collection_mark_unmodifiable(handle: i64) -> i64 {
    let e_lista = HEAP.with(|h| {
        let h_ = h.borrow();
        h_.e_lista(handle) && h_.classe(handle) != crate::layout::cid::IMMUTABLE_LIST
    });
    if !e_lista {
        return handle;
    }
    // `handle` é argumento: o código gerado o mantém enraizado.
    let novo = HEAP.with(|h| h.borrow_mut().lista_imutavel_de(handle));
    tipar_lista_copiada(novo, handle, crate::layout::cid::IMMUTABLE_LIST);
    novo
}
