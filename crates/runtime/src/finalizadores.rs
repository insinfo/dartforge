// Runtime nativo: `Finalizer` (dart:core) e `NativeFinalizer` (dart:ffi) —
// o `FinalizerEntry` da VM (`runtime/vm/object.h`, `gc_marker.cc`) sobre os
// anexos do coletor (`Heap::attachments`).
//
// * Um anexo guarda o valor e a chave de `detach` como referências fracas;
//   a ação é aresta forte do dono, que o registro não enraíza. A coleta que acha o valor morto
//   tira o anexo: a ação de um `Finalizer` (a closure `() => callback(token)`
//   montada no Dart) vai para a fila de finalizações prontas, que o laço de
//   eventos atende entre um evento e outro — na VM é uma mensagem ao
//   isolado; a de um `NativeFinalizer` (`callback(token)`, função C) roda na
//   própria coleta, como na VM, e não toca o heap.
// * `detach(key)` tira os anexos do mesmo dono com a mesma chave.
// * No fim do isolado os `NativeFinalizer` ainda anexados rodam (a VM os
//   garante no encerramento); os `Finalizer`, não (a especificação não
//   promete que rodem).

/// `DartForge_finalizador_anexar(owner, value, action, detach)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_finalizador_anexar(owner: i64, value: i64, action: i64, detach: i64) {
    HEAP.with(|h| {
        h.borrow_mut().add_attachment(crate::heap::FinalizerAttachment {
            owner,
            value,
            detach,
            action: crate::heap::FinalizerAction::Dart(action),
        })
    });
}

/// `DartForge_finalizador_anexar_nativo(dono, valor, funcao, token,
/// desanexo, tamanho_externo)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_finalizador_anexar_nativo(owner: i64, value: i64, function: i64, token: i64, detach: i64, external_size: i64) {
    if function == 0 {
        throw_argument_error("NativeFinalizer callback must not be nullptr");
        return;
    }
    // NativeFinalizer pode rodar fora da thread do isolado. Um trampolim
    // síncrono entraria em Dart durante a coleta ou abortaria noutra thread.
    if callback_requires_isolate_thread(function as usize) {
        throw_argument_error("NativeFinalizer callback must not require the isolate thread");
        return;
    }
    if external_size < 0 || usize::try_from(external_size).ok().and_then(|n| isize::try_from(n).ok()).is_none() {
        throw_argument_error("externalSize fora do intervalo suportado");
        return;
    }
    HEAP.with(|h| {
        h.borrow_mut().add_attachment(crate::heap::FinalizerAttachment {
            owner,
            value,
            detach,
            action: crate::heap::FinalizerAction::Native(function as usize, token as usize, external_size as usize),
        })
    });
}

/// `DartForge_finalizador_desanexar(owner, detach)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_finalizador_desanexar(owner: i64, detach: i64) {
    if detach == 0 {
        return;
    }
    HEAP.with(|h| h.borrow_mut().detach_finalizer(owner, detach));
}

/// Consome a entrada antes de chamar Dart; reentrada só encontra ações pendentes.
fn start_finalization() -> Option<(i64, i64)> {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        if h.ready_finalizations.is_empty() { return None; }
        let frame = h.push_owning_frame(1);
        let action = h.move_finalization_to_slot(frame, 0);
        Some((frame, action))
    })
}

fn finish_finalization(frame: i64) {
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
}

/// Fim do isolado: os `NativeFinalizer` ainda anexados rodam.
pub fn shutdown_isolate_finalizers() {
    HEAP.with(|h| h.borrow_mut().encerrar_finalizadores());
}

#[cfg(test)]
mod tests_finalization_owner {
    use super::*;

    thread_local! {
        static ACTIVE_COUNT: std::cell::RefCell<Vec<i64>> = const { std::cell::RefCell::new(Vec::new()) };
        static EXECUTED: std::cell::RefCell<Vec<i64>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    extern "C" fn run_with_reentry(closure: i64) -> i64 {
        EXECUTED.with(|e| {
            let mut e = e.borrow_mut();
            assert!(!e.contains(&closure), "cada entrada deve executar uma vez");
            e.push(closure);
        });
        ACTIVE_COUNT.with(|a| a.borrow_mut().push(closure));
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            assert!(!h.ready_finalizations.contains(&closure));
            h.collect();
            ACTIVE_COUNT.with(|a| for &active_one in a.borrow().iter() {
                assert!(h.is_live_object(active_one));
            });
        });
        dartforge_laco_de_eventos(run_with_reentry);
        ACTIVE_COUNT.with(|a| { assert_eq!(a.borrow_mut().pop(), Some(closure)); });
        0
    }

    #[test]
    fn reentrant_finalizations_consume_each_entry_once() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        EXECUTED.with(|e| e.borrow_mut().clear());
        let closures = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            let first_one = h.new_closure(123, (0, false), 0, 0);
            h.add_ready_finalization(first_one);
            let second_one = h.new_closure(456, (0, false), 0, 0);
            h.add_ready_finalization(second_one);
            [first_one, second_one]
        });
        dartforge_laco_de_eventos(run_with_reentry);
        EXECUTED.with(|e| assert_eq!(&*e.borrow(), &closures));
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            assert!(h.ready_finalizations.is_empty());
            h.collect();
            for closure in closures { assert!(!h.is_live_object(closure)); }
        });
        HEAP.with(|h| { h.replace(previous); });
    }

    extern "C" fn shut_down_during_callback(closure: i64) -> i64 {
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.encerrar_finalizadores();
            h.collect();
            assert!(h.is_live_object(closure), "o callback ativo sobrevive à retirada da fila");
        });
        0
    }

    #[test]
    fn active_finalization_survives_reentrant_shutdown() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let closure = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            let closure = h.new_closure(123, (0, false), 0, 0);
            h.add_ready_finalization(closure);
            closure
        });
        dartforge_laco_de_eventos(shut_down_during_callback);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            assert!(h.ready_finalizations.is_empty());
            h.collect();
            assert!(!h.is_live_object(closure));
        });
        HEAP.with(|h| { h.replace(previous); });
    }
}
