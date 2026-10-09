// ABI versionada dos slots proprietários usados pelo código gerado.
// A pilha observacional não substitui estes owners. Cópia/movimento/fecho
// não coletam, não executam Dart e mantêm o inventário auditável do Heap.

/// Abre um quadro proprietário com slots inicialmente nulos.
///
/// # Panics
/// Quantidade negativa/incompatível com usize ou falha interna; o limite C
/// aborta, sem desenrolar para o código gerado.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_quadro_abrir_v1, dartforge_arc_quadro_fechar_v1};
/// let quadro = dartforge_arc_quadro_abrir_v1(0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_abrir_v1(slots: i64) -> i64 {
    let slots = usize::try_from(slots).expect("quantidade inválida de slots ARC");
    HEAP.with(|h| h.borrow_mut().push_frame_proprietario(slots))
}

/// Copia o valor para um slot, retendo antes de soltar o conteúdo antigo.
///
/// O quadro e índice são escalares; valor é Ref gerenciado, inclusive null/Smi.
/// Zero consome o conteúdo anterior. Não atende dívida de coleta.
///
/// # Panics
/// Quadro/slot inválido ou referência morta; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_copiar_v1(quadro, 0, 0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_copiar_v1(quadro: i64, slot: i64, valor: i64) {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.conferir_quadro_proprietario(quadro);
        h.set_root(quadro, slot, valor);
    });
}

/// Move um owner entre slots, consumindo origem e conteúdo antigo do destino.
///
/// Mover para o mesmo slot preserva a ocorrência. Não há coleta nem Dart
/// entre publicação do destino e limpeza da origem.
///
/// # Panics
/// Quadros não proprietários, inexistentes ou índices inválidos; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let quadro = dartforge_arc_quadro_abrir_v1(2);
/// dartforge_arc_quadro_mover_v1(quadro, 0, quadro, 1);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_mover_v1(
    origem: i64,
    slot_origem: i64,
    destino: i64,
    slot_destino: i64,
) {
    let de = usize::try_from(slot_origem).expect("índice inválido de origem ARC");
    let para = usize::try_from(slot_destino).expect("índice inválido de destino ARC");
    HEAP.with(|h| h.borrow_mut().mover_raiz(origem, de, destino, para));
}

/// Consome os owners do quadro do topo sem coletar ou executar Dart.
///
/// # Panics
/// Quadro não está no topo; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_fechar_v1(quadro: i64) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.conferir_quadro_proprietario(quadro);
        h.pop_frame(quadro);
    });
}

#[cfg(test)]
mod testes_arc_abi_quadros {
    use super::*;

    #[test]
    #[should_panic(expected = "quadro ARC deve ser proprietário")]
    fn abi_recusa_quadro_observacional_antes_de_alterar_slots() {
        let mut heap = Heap::new(false);
        let quadro = heap.push_frame_with_slots(1);
        heap.conferir_quadro_proprietario(quadro);
    }

    #[test]
    fn copia_movimento_e_fecho_preservam_occorrencias_ate_o_ultimo_owner() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(false)));
            if arc {
                HEAP.with(|h| h.borrow_mut().ativar_arc());
            }
            let origem = dartforge_arc_quadro_abrir_v1(2);
            let destino = dartforge_arc_quadro_abrir_v1(1);
            let valor = HEAP.with(|h| h.borrow_mut().alocar_str("owner ABI"));
            dartforge_arc_quadro_copiar_v1(origem, 0, valor);
            dartforge_arc_quadro_copiar_v1(origem, 1, valor);
            dartforge_arc_quadro_mover_v1(origem, 0, destino, 0);
            dartforge_arc_quadro_mover_v1(destino, 0, destino, 0);
            dartforge_arc_quadro_copiar_v1(origem, 1, 0);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert!(h.e_objeto_vivo(valor));
            });
            dartforge_arc_quadro_fechar_v1(destino);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert!(!h.e_objeto_vivo(valor));
            });
            dartforge_arc_quadro_fechar_v1(origem);
            HEAP.with(|h| {
                h.replace(anterior);
            });
        }
    }
}
