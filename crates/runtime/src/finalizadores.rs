// Runtime nativo: `Finalizer` (dart:core) e `NativeFinalizer` (dart:ffi) —
// o `FinalizerEntry` da VM (`runtime/vm/object.h`, `gc_marker.cc`) sobre os
// anexos do coletor (`Heap::anexos`).
//
// * Um anexo guarda o valor e a chave de `detach` como referências fracas;
//   a ação é aresta forte do dono, que o registro não enraíza. A coleta que acha o valor morto
//   tira o anexo: a ação de um `Finalizer` (a closure `() => callback(token)`
//   montada no Dart) vai para a fila de finalizações prontas, que o laço de
//   eventos atende entre um evento e outro — na VM é uma mensagem ao
//   isolado; a de um `NativeFinalizer` (`callback(token)`, função C) roda na
//   própria coleta, como na VM, e não toca o heap.
// * `detach(chave)` tira os anexos do mesmo dono com a mesma chave.
// * No fim do isolado os `NativeFinalizer` ainda anexados rodam (a VM os
//   garante no encerramento); os `Finalizer`, não (a especificação não
//   promete que rodem).

/// `DartForge_finalizador_anexar(dono, valor, acao, desanexo)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_finalizador_anexar(dono: i64, valor: i64, acao: i64, desanexo: i64) {
    HEAP.with(|h| {
        h.borrow_mut().adicionar_anexo(crate::heap::AnexoDeFinalizador {
            dono,
            valor,
            desanexo,
            acao: crate::heap::AcaoDeFinalizador::Dart(acao),
        })
    });
}

/// `DartForge_finalizador_anexar_nativo(dono, valor, funcao, token,
/// desanexo, tamanho_externo)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_finalizador_anexar_nativo(dono: i64, valor: i64, funcao: i64, token: i64, desanexo: i64, tamanho_externo: i64) {
    if funcao == 0 {
        lancar_erro_de_argumento("NativeFinalizer callback must not be nullptr");
        return;
    }
    if tamanho_externo < 0 || usize::try_from(tamanho_externo).ok().and_then(|n| isize::try_from(n).ok()).is_none() {
        lancar_erro_de_argumento("externalSize fora do intervalo suportado");
        return;
    }
    HEAP.with(|h| {
        h.borrow_mut().adicionar_anexo(crate::heap::AnexoDeFinalizador {
            dono,
            valor,
            desanexo,
            acao: crate::heap::AcaoDeFinalizador::Nativa(funcao as usize, token as usize, tamanho_externo as usize),
        })
    });
}

/// `DartForge_finalizador_desanexar(dono, desanexo)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_finalizador_desanexar(dono: i64, desanexo: i64) {
    if desanexo == 0 {
        return;
    }
    HEAP.with(|h| h.borrow_mut().desanexar_finalizador(dono, desanexo));
}

/// Consome a entrada antes de chamar Dart; reentrada só encontra ações pendentes.
fn iniciar_finalizacao() -> Option<(i64, i64)> {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        if h.finalizacoes_prontas.is_empty() { return None; }
        let quadro = h.push_frame_proprietario(1);
        let acao = h.mover_finalizacao_para_slot(quadro, 0);
        Some((quadro, acao))
    })
}

fn concluir_finalizacao(quadro: i64) {
    HEAP.with(|h| h.borrow_mut().pop_frame(quadro));
}

/// Fim do isolado: os `NativeFinalizer` ainda anexados rodam.
pub fn encerrar_finalizadores_do_isolado() {
    HEAP.with(|h| h.borrow_mut().encerrar_finalizadores());
}

#[cfg(test)]
mod testes_owner_finalizacao {
    use super::*;

    thread_local! {
        static ATIVAS: std::cell::RefCell<Vec<i64>> = const { std::cell::RefCell::new(Vec::new()) };
        static EXECUTADAS: std::cell::RefCell<Vec<i64>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    extern "C" fn executar_com_reentrada(closure: i64) -> i64 {
        EXECUTADAS.with(|e| {
            let mut e = e.borrow_mut();
            assert!(!e.contains(&closure), "cada entrada deve executar uma vez");
            e.push(closure);
        });
        ATIVAS.with(|a| a.borrow_mut().push(closure));
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            assert!(!h.finalizacoes_prontas.contains(&closure));
            h.collect();
            ATIVAS.with(|a| for &ativa in a.borrow().iter() {
                assert!(h.e_objeto_vivo(ativa));
            });
        });
        dartforge_laco_de_eventos(executar_com_reentrada);
        ATIVAS.with(|a| { assert_eq!(a.borrow_mut().pop(), Some(closure)); });
        0
    }

    #[test]
    fn finalizacoes_reentrantes_consumem_cada_entrada_uma_vez() {
        let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
        EXECUTADAS.with(|e| e.borrow_mut().clear());
        let closures = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.ativar_arc();
            let primeira = h.nova_closure(123, (0, false), 0, 0);
            h.adicionar_finalizacao_pronta(primeira);
            let segunda = h.nova_closure(456, (0, false), 0, 0);
            h.adicionar_finalizacao_pronta(segunda);
            [primeira, segunda]
        });
        dartforge_laco_de_eventos(executar_com_reentrada);
        EXECUTADAS.with(|e| assert_eq!(&*e.borrow(), &closures));
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            assert!(h.finalizacoes_prontas.is_empty());
            h.collect();
            for closure in closures { assert!(!h.e_objeto_vivo(closure)); }
        });
        HEAP.with(|h| { h.replace(anterior); });
    }

    extern "C" fn encerrar_durante_callback(closure: i64) -> i64 {
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.encerrar_finalizadores();
            h.collect();
            assert!(h.e_objeto_vivo(closure), "o callback ativo sobrevive à retirada da fila");
        });
        0
    }

    #[test]
    fn finalizacao_ativa_sobrevive_ao_encerramento_reentrante() {
        let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
        let closure = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.ativar_arc();
            let closure = h.nova_closure(123, (0, false), 0, 0);
            h.adicionar_finalizacao_pronta(closure);
            closure
        });
        dartforge_laco_de_eventos(encerrar_durante_callback);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            assert!(h.finalizacoes_prontas.is_empty());
            h.collect();
            assert!(!h.e_objeto_vivo(closure));
        });
        HEAP.with(|h| { h.replace(anterior); });
    }
}
