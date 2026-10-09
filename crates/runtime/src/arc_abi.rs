// ABI de tokens e slots proprietários usados pelo código gerado.
// A pilha observacional não substitui estes owners. Operações de ownership
// não coletam nem executam Dart; coleta tem um safepoint explícito separado.

/// Cria um token proprietário do código gerado a partir de um empréstimo.
///
/// Null e Smi não têm contador. Não coleta nem executa Dart. O token fica
/// no inventário do heap até release; slots proprietários são independentes.
///
/// # Panics
/// Handle morto ou overflow; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_retain, dartforge_arc_release};
/// dartforge_arc_retain(0);
/// dartforge_arc_release(0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_retain(valor: i64) {
    HEAP.with(|h| h.borrow_mut().reter_owner_codigo(valor));
}

/// Consome um token do código gerado, sem coletar ou executar Dart.
///
/// Não consome owners de quadros, campos ou mensagens. Null/Smi são no-op.
/// Zero de RC apenas agenda trabalho para um safepoint posterior.
///
/// # Panics
/// Handle sem token correspondente; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_retain, dartforge_arc_release};
/// dartforge_arc_retain(0);
/// dartforge_arc_release(0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_release(valor: i64) {
    HEAP.with(|h| h.borrow_mut().soltar_owner_codigo(valor));
}

/// Executa coleta explícita num safepoint com as raízes já publicadas.
///
/// Pode executar callbacks nativos de finalização; callbacks Dart são
/// enfileirados, sem execução nesta chamada. Em tracing faz coleta completa.
///
/// # Panics
/// Falha interna de coleta; aborta no limite C.
///
/// ```
/// dartforge_runtime::abi::dartforge_arc_collect();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_collect() {
    HEAP.with(|h| h.borrow_mut().collect());
}

/// Confere a versão de tokens desta ABI e o modo ARC do heap corrente.
///
/// Retorna 1 somente para versão 1 com ARC ativo, 0 para incompatibilidade.
/// O chamador deve recusar a execução ARC quando o resultado for zero.
/// Não certifica contratos de um módulo nem a ABI de retornos das externs.
///
/// ```
/// assert_eq!(dartforge_runtime::abi::dartforge_arc_verificar_abi(-1), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_verificar_abi(versao: i64) -> i8 {
    i8::from(versao == 1 && HEAP.with(|h| h.borrow().arc_ativo()))
}

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

/// Carrega um slot proprietário e cria um token owned do código.
///
/// O slot mantém sua ocorrência. O resultado precisa de release ou transferência,
/// mesmo se o quadro for fechado antes. Não coleta nem executa Dart.
///
/// # Panics
/// Quadro observacional/inexistente, slot inválido ou falha interna; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// let valor = dartforge_arc_quadro_carregar_v1(quadro, 0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// dartforge_arc_release(valor);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_carregar_v1(quadro: i64, slot: i64) -> i64 {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| h.borrow_mut().copiar_raiz_para_codigo(quadro, slot))
}

/// Transfere um token do código para um slot proprietário.
///
/// Não retém a origem. Publica o destino antes de soltar seu owner anterior.
/// Null/Smi não exigem token físico. Não coleta nem executa Dart.
///
/// # Panics
/// Quadro/slot inválido ou handle sem token do código; valida antes da mutação.
/// Falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_receber_v1(quadro, 0, 0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_receber_v1(quadro: i64, slot: i64, valor: i64) {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| h.borrow_mut().mover_codigo_para_raiz(quadro, slot, valor));
}

/// Transfere token do código ao registro de owner de um global.
///
/// `id` identifica o armazenamento, não é desreferenciado. O código gerado
/// deve publicar o valor real antes, sem safepoint até esta chamada. O registro
/// guarda apenas owners de handles; não fornece o valor de uma carga global.
/// Não retém a origem, coleta ou executa Dart. Null/Smi retiram owner anterior.
///
/// # Panics
/// Handle morto ou sem token do código; falha interna aborta no limite C.
///
/// ```
/// dartforge_runtime::abi::dartforge_arc_global_receber_v1(17, 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_global_receber_v1(id: i64, valor: i64) {
    HEAP.with(|h| h.borrow_mut().mover_codigo_para_global(id, valor));
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
    fn abi_carrega_owner_independente_e_recebe_token_no_slot() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(false)));
            if arc {
                HEAP.with(|h| h.borrow_mut().ativar_arc());
            }
            let quadro = dartforge_arc_quadro_abrir_v1(1);
            let valor = HEAP.with(|h| h.borrow_mut().alocar_str("slot e SSA"));
            dartforge_arc_quadro_copiar_v1(quadro, 0, valor);
            let token = dartforge_arc_quadro_carregar_v1(quadro, 0);
            assert_eq!(token, valor);
            dartforge_arc_quadro_receber_v1(quadro, 0, token);
            let token = dartforge_arc_quadro_carregar_v1(quadro, 0);
            dartforge_arc_quadro_fechar_v1(quadro);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(token)));
            dartforge_arc_release(token);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(token)));
            let quadro = dartforge_arc_quadro_abrir_v1(1);
            for escalar in [0, smi::de(42).unwrap()] {
                dartforge_arc_quadro_receber_v1(quadro, 0, escalar);
                assert_eq!(dartforge_arc_quadro_carregar_v1(quadro, 0), escalar);
                dartforge_arc_release(escalar);
            }
            dartforge_arc_quadro_fechar_v1(quadro);
            HEAP.with(|h| {
                h.replace(anterior);
            });
        }
    }

    #[test]
    fn abi_minima_confere_modo_e_sustenta_tokens_ate_release() {
        let anterior = HEAP.with(|h| h.replace(Heap::new(false)));
        assert_eq!(dartforge_arc_verificar_abi(1), 0);
        HEAP.with(|h| h.borrow_mut().ativar_arc());
        assert_eq!(dartforge_arc_verificar_abi(1), 1);
        assert_eq!(dartforge_arc_verificar_abi(2), 0);
        let valor = HEAP.with(|h| h.borrow_mut().alocar_str("token ABI"));
        dartforge_arc_retain(valor);
        dartforge_arc_retain(valor);
        dartforge_arc_collect();
        assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
        dartforge_arc_release(valor);
        dartforge_arc_collect();
        assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
        dartforge_arc_release(valor);
        dartforge_arc_collect();
        assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
        HEAP.with(|h| {
            h.replace(anterior);
        });
    }

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
