// Runtime nativo: o laço de eventos do isolado (P6, docs/NATIVO-PLANO.md
// §7.7) — microtarefas e timers.
//
// A fila de microtarefas e a ordem delas são as do `dart:async` da fonte
// (`schedule_microtask.dart`: `_nextCallback`, `_startMicrotaskLoop`); o
// runtime só guarda as closures que `_AsyncRun._scheduleImmediate` entrega
// (o native `DartForge_scheduleImmediate` da sobreposição `native_sdk/`) e as
// roda, em ordem, antes de qualquer timer. Os timers são os da VM
// (`_internal/vm/lib/timer_impl.dart`, a referência): prazo em milissegundos
// de um relógio monotônico — `current_time` para duração 0 e `current_time + 1 + ms` para
// as outras —, desempate pela ordem de criação (o `_id` da VM), microtarefas
// esvaziadas depois de cada timer, periódico reagendado em `deadline + ms`.
//
// É o ÚNICO ponto em que o runtime chama código Dart: pela função que o
// código gerado entrega em `dartforge_laco_de_eventos` (`call`, que chama
// uma closure sem argumentos pela convenção uniforme e devolve com a exceção
// pendente, se houver). Exceção que sai de uma microtarefa ou de um timer não
// foi tratada pela `Zone` (o `_rootHandleError` da fonte a relança num
// `Error.throwWithStackTrace`): o laço para e `finalizar_programa` a relata,
// como a VM faz com "Unhandled exception".
//
// As closures guardadas são raízes do coletor enquanto esperam: cada uma
// ocupa um global de raiz do heap (`set_global_root`) com id NEGATIVO — os
// globais do programa usam os ids não negativos.
//
// Estado por thread (um isolado por thread; P8 junta tudo em `isolado.rs`).

struct LoopTimer {
    /// A closure que dispara o timer (`_Timer._disparar` com o receptor).
    closure: i64,
    root: i64,
    ms: i64,
    periodic: bool,
}

#[derive(Default)]
struct Events {
    /// Closures entregues por `DartForge_scheduleImmediate`, com a raiz.
    immediates: std::collections::VecDeque<(i64, i64)>,
    /// Timers agendados por `(deadline at ms, sequência)` → id. A sequência é
    /// o `_id` da VM: cresce a cada agendamento (e reagendamento).
    queue: std::collections::BTreeMap<(i64, i64), i64>,
    next_seq: i64,
    /// Timers ativos por id (o de `DartForge_Timer_novo`, nunca 0).
    active: crate::hash::HashMap<i64, LoopTimer>,
    next_id: i64,
    next_root: i64,
    start: Option<std::time::Instant>,
}

thread_local! {
    static EVENTS: RefCell<Events> = RefCell::new(Events::default());
}

impl Events {
    /// Milissegundos do relógio monotônico do isolado.
    fn current_time(&mut self) -> i64 {
        let start = *self.start.get_or_insert_with(std::time::Instant::now);
        i64::try_from(start.elapsed().as_millis()).unwrap_or(i64::MAX)
    }

    /// Uma raiz nova para `handle` (id negativo, ver o cabeçalho).
    fn root_it(&mut self, handle: i64) -> i64 {
        self.next_root += 1;
        let id = -self.next_root;
        HEAP.with(|h| h.borrow_mut().set_global_root(id, handle));
        id
    }
}

fn release_root(id: i64) {
    HEAP.with(|h| h.borrow_mut().set_global_root(id, 0));
}

/// O evento único move seu owner da fila; o periódico empresta da cópia ativa,
/// que continua viva mesmo se o callback cancelar o owner persistente do timer.
fn call_event(call: extern "C" fn(i64) -> i64, closure: i64, root: Option<i64>) {
    let frame = HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let frame = h.push_owning_frame(1);
        if let Some(root) = root {
            h.move_global_to_slot(root, frame, 0);
        } else {
            h.set_root(frame, 0, closure);
        }
        frame
    });
    dart_r1(call as usize, closure);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
}

/// `_AsyncRun._scheduleImmediate(callback)`: a closure roda antes do
/// próximo timer, depois das que já esperam.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_scheduleImmediate(closure: i64) {
    EVENTS.with(|e| {
        let mut e = e.borrow_mut();
        let root = e.root_it(closure);
        e.immediates.push_back((root, closure));
    });
}

/// `_Timer._novo(ms, fire, periodic)`: agenda o timer e devolve o id
/// (nunca 0). `fire` é a closure que o laço chama no prazo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_Timer_novo(ms: i64, fire: i64, periodic: u8) -> i64 {
    EVENTS.with(|e| {
        let mut e = e.borrow_mut();
        let ms = ms.max(0);
        let current_time = e.current_time();
        let deadline = if ms == 0 { current_time } else { current_time + 1 + ms };
        e.next_id += 1;
        let id = e.next_id;
        let root = e.root_it(fire);
        e.next_seq += 1;
        let seq = e.next_seq;
        e.queue.insert((deadline, seq), id);
        e.active.insert(id, LoopTimer { closure: fire, root, ms, periodic: periodic != 0 });
        id
    })
}

/// `_Timer._cancelar(id)`: tira o timer da fila e solta a raiz.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_Timer_cancelar(id: i64) {
    let root = EVENTS.with(|e| e.borrow_mut().active.remove(&id).map(|t| t.root));
    if let Some(r) = root {
        release_root(r);
    }
}

/// O laço de eventos: roda as microtarefas pendentes e os timers até não
/// haver mais nada (ou uma exceção sair de um deles). `call` é a função do
/// código gerado que chama uma closure sem argumentos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_laco_de_eventos(call: extern "C" fn(i64) -> i64) {
    // No laço, o isolado atende pedidos no ponto seguro (`portas.rs`).
    register_live_isolate();
    loop {
        if dartforge_exception_pending() != 0 {
            return;
        }
        // 1. Microtarefas, todas, antes de qualquer timer.
        let immediate_one = EVENTS.with(|e| e.borrow_mut().immediates.pop_front());
        if let Some((root, closure)) = immediate_one {
            // Cada evento no próprio escopo nativo: os owners que as portas
            // Dart devolvem morrem no fim do evento, não do laço.
            in_native_scope_if_arc(|| call_event(call, closure, Some(root)));
            continue;
        }
        // 1b. As finalizações prontas (`Finalizer`, `finalizadores.rs`):
        //     na VM chegam como mensagem ao isolado; aqui, entre eventos.
        if let Some((frame, action)) = start_finalization() {
            in_native_scope_if_arc(|| dart_r1(call as usize, action));
            finish_finalization(frame);
            continue;
        }
        // 2. Os pedidos no ponto seguro (a publicação de uma recarga do JIT,
        //    `portas.rs`) e as mensagens de controle (`isolados.rs`), entre
        //    um evento e outro; pausado, o isolado só atende os dois.
        serve_safepoints();
        if !serve_control() {
            return;
        }
        if paused_isolate() {
            wait_message(None, true);
            continue;
        }
        // 3. O próximo evento: o timer de menor (prazo, id) ou a mensagem
        //    mais antiga da fila do isolado (`portas.rs`), pela ordem de
        //    chegada — na VM o timer também chega como mensagem. Sem nenhum
        //    dos dois, o isolado espera enquanto tiver porta viva.
        let next_offset = EVENTS.with(|e| {
            let mut e = e.borrow_mut();
            loop {
                let (&(deadline, seq), &id) = e.queue.iter().next()?;
                if e.active.contains_key(&id) {
                    return Some((deadline, seq, id));
                }
                // Cancelado: a entrada da fila é descartada.
                e.queue.remove(&(deadline, seq));
            }
        });
        let due = next_offset.map(|(deadline, _, _)| {
            EVENTS.with(|e| {
                let mut e = e.borrow_mut();
                e.current_time();
                e.start.expect("relógio iniciado") + std::time::Duration::from_millis(deadline.max(0) as u64)
            })
        });
        if let Some(arrival) = next_arrival()
            && due.is_none_or(|v| arrival < v)
        {
            in_native_scope_if_arc(|| dispatch_next(call));
            continue;
        }
        let Some((deadline, seq, id)) = next_offset else {
            if has_live_port() {
                wait_message(None, false);
                continue;
            }
            return;
        };
        let due = due.expect("timer com prazo");
        if std::time::Instant::now() < due {
            // Espera o prazo, acordando antes se chegar mensagem.
            wait_message(Some(due), false);
            continue;
        }
        EVENTS.with(|e| e.borrow_mut().queue.remove(&(deadline, seq)));
        let firing = EVENTS.with(|e| {
            let mut e = e.borrow_mut();
            let t = e.active.get(&id)?;
            let (closure, periodic) = (t.closure, t.periodic);
            if periodic {
                return Some((closure, true, None));
            }
            let t = e.active.remove(&id).expect("timer ativo");
            Some((closure, false, Some(t.root)))
        });
        let Some((closure, periodic, root)) = firing else {
            continue;
        };
        in_native_scope_if_arc(|| call_event(call, closure, root));
        if periodic {
            // Como a VM (`_runTimers`): reagendado DEPOIS do callback, com
            // sequência nova, se o callback não o cancelou.
            EVENTS.with(|e| {
                let mut e = e.borrow_mut();
                let Some(ms) = e.active.get(&id).map(|t| t.ms) else { return };
                let new = if ms > 0 { deadline + ms } else { e.current_time() };
                e.next_seq += 1;
                let seq = e.next_seq;
                e.queue.insert((new, seq), id);
            });
        }
    }
}

/// `_trySetStackTrace(error, stackTrace)` (`Error_trySetStackTrace` da
/// VM): grava o rastro num `Error` que ainda não tem um. Os erros do programa
/// e da fonte guardam o rastro no próprio layout (o `Error` da fonte, P5);
/// os do runtime (ids 1000–1012) já o recebem no `throw`. Até o `Error` vir
/// da fonte, não há o que gravar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Error_trySetStackTrace(_error: i64, _trace: i64) {}

#[cfg(test)]
mod tests_event_owners {
    use super::*;
    thread_local! { static TIMER: std::cell::Cell<i64> = const { std::cell::Cell::new(0) }; }

    extern "C" fn callback(closure: i64) -> i64 {
        let timer = TIMER.with(|t| t.get());
        if timer != 0 { dartforge_nativo_DartForge_Timer_cancelar(timer); }
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(h.is_live_object(closure), "callback perdeu owner durante a chamada");
        });
        0
    }

    fn verify(periodic: bool) {
        let previous_heap = HEAP.with(|h| h.replace(Heap::new(true)));
        let previous_events = EVENTS.with(|e| e.replace(Events::default()));
        let closure = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            h.new_closure(123, (0, false), 0, 0)
        });
        if periodic {
            let id = dartforge_nativo_DartForge_Timer_novo(0, closure, 1);
            TIMER.with(|t| t.set(id));
            call_event(callback, closure, None);
        } else {
            TIMER.with(|t| t.set(0));
            dartforge_nativo_DartForge_scheduleImmediate(closure);
            let (root, value) = EVENTS.with(|e| e.borrow_mut().immediates.pop_front().unwrap());
            call_event(callback, value, Some(root));
        }
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.is_live_object(closure), "owner ativo residual após o callback");
        });
        TIMER.with(|t| t.set(0));
        EVENTS.with(|e| { e.replace(previous_events); });
        HEAP.with(|h| { h.replace(previous_heap); });
    }

    #[test]
    fn single_event_transfers_queue_owner_and_releases_after_callback() { verify(false); }

    #[test]
    fn periodic_timer_cancelled_in_callback_keeps_active_owner() { verify(true); }

    extern "C" fn callback_throws(closure: i64) -> i64 {
        dartforge_exception_throw(closure, 3);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(h.is_live_object(closure));
        });
        0
    }

    #[test]
    fn callback_with_exception_releases_active_owner_and_keeps_pending() {
        let previous_heap = HEAP.with(|h| h.replace(Heap::new(true)));
        let previous_events = EVENTS.with(|e| e.replace(Events::default()));
        let closure = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            h.new_closure(123, (0, false), 0, 0)
        });
        dartforge_nativo_DartForge_scheduleImmediate(closure);
        let (root, value) = EVENTS.with(|e| e.borrow_mut().immediates.pop_front().unwrap());
        call_event(callback_throws, value, Some(root));
        assert_eq!(dartforge_exception_pending(), 1);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(h.is_live_object(closure), "a exceção pendente deve guardar seu owner");
        });
        dartforge_exception_clear();
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.is_live_object(closure), "owner ativo ou da fila ficou residual");
        });
        EVENTS.with(|e| { e.replace(previous_events); });
        HEAP.with(|h| { h.replace(previous_heap); });
    }
}
