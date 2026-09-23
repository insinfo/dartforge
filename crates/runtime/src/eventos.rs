// Runtime nativo: o laço de eventos do isolado (P6, docs/NATIVO-PLANO.md
// §7.7) — microtarefas e timers.
//
// A fila de microtarefas e a ordem delas são as do `dart:async` da fonte
// (`schedule_microtask.dart`: `_nextCallback`, `_startMicrotaskLoop`); o
// runtime só guarda as closures que `_AsyncRun._scheduleImmediate` entrega
// (o native `DartForge_scheduleImmediate` da sobreposição `sdk_nativo/`) e as
// roda, em ordem, antes de qualquer timer. Os timers são os da VM
// (`_internal/vm/lib/timer_impl.dart`, a referência): prazo em milissegundos
// de um relógio monotônico — `agora` para duração 0 e `agora + 1 + ms` para
// as outras —, desempate pela ordem de criação (o `_id` da VM), microtarefas
// esvaziadas depois de cada timer, periódico reagendado em `prazo + ms`.
//
// É o ÚNICO ponto em que o runtime chama código Dart: pela função que o
// código gerado entrega em `dartforge_laco_de_eventos` (`chamar`, que chama
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

struct TimerDoLaco {
    /// A closure que dispara o timer (`_Timer._disparar` com o receptor).
    closure: i64,
    raiz: i64,
    ms: i64,
    periodico: bool,
}

#[derive(Default)]
struct Eventos {
    /// Closures entregues por `DartForge_scheduleImmediate`, com a raiz.
    imediatas: std::collections::VecDeque<(i64, i64)>,
    /// Timers agendados por `(prazo em ms, sequência)` → id. A sequência é
    /// o `_id` da VM: cresce a cada agendamento (e reagendamento).
    fila: std::collections::BTreeMap<(i64, i64), i64>,
    prox_seq: i64,
    /// Timers ativos por id (o de `DartForge_Timer_novo`, nunca 0).
    ativos: std::collections::HashMap<i64, TimerDoLaco>,
    prox_id: i64,
    prox_raiz: i64,
    inicio: Option<std::time::Instant>,
}

thread_local! {
    static EVENTOS: RefCell<Eventos> = RefCell::new(Eventos::default());
}

impl Eventos {
    /// Milissegundos do relógio monotônico do isolado.
    fn agora(&mut self) -> i64 {
        let inicio = *self.inicio.get_or_insert_with(std::time::Instant::now);
        i64::try_from(inicio.elapsed().as_millis()).unwrap_or(i64::MAX)
    }

    /// Uma raiz nova para `handle` (id negativo, ver o cabeçalho).
    fn enraizar(&mut self, handle: i64) -> i64 {
        self.prox_raiz += 1;
        let id = -self.prox_raiz;
        HEAP.with(|h| h.borrow_mut().set_global_root(id, handle));
        id
    }
}

fn soltar_raiz(id: i64) {
    HEAP.with(|h| h.borrow_mut().set_global_root(id, 0));
}

/// `_AsyncRun._scheduleImmediate(callback)`: a closure roda antes do
/// próximo timer, depois das que já esperam.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_scheduleImmediate(closure: i64) {
    EVENTOS.with(|e| {
        let mut e = e.borrow_mut();
        let raiz = e.enraizar(closure);
        e.imediatas.push_back((raiz, closure));
    });
}

/// `_Timer._novo(ms, disparar, periodico)`: agenda o timer e devolve o id
/// (nunca 0). `disparar` é a closure que o laço chama no prazo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_Timer_novo(ms: i64, disparar: i64, periodico: u8) -> i64 {
    EVENTOS.with(|e| {
        let mut e = e.borrow_mut();
        let ms = ms.max(0);
        let agora = e.agora();
        let prazo = if ms == 0 { agora } else { agora + 1 + ms };
        e.prox_id += 1;
        let id = e.prox_id;
        let raiz = e.enraizar(disparar);
        e.prox_seq += 1;
        let seq = e.prox_seq;
        e.fila.insert((prazo, seq), id);
        e.ativos.insert(id, TimerDoLaco { closure: disparar, raiz, ms, periodico: periodico != 0 });
        id
    })
}

/// `_Timer._cancelar(id)`: tira o timer da fila e solta a raiz.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_Timer_cancelar(id: i64) {
    let raiz = EVENTOS.with(|e| e.borrow_mut().ativos.remove(&id).map(|t| t.raiz));
    if let Some(r) = raiz {
        soltar_raiz(r);
    }
}

/// O laço de eventos: roda as microtarefas pendentes e os timers até não
/// haver mais nada (ou uma exceção sair de um deles). `chamar` é a função do
/// código gerado que chama uma closure sem argumentos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_laco_de_eventos(chamar: extern "C" fn(i64) -> i64) {
    loop {
        if dartforge_exception_pending() != 0 {
            return;
        }
        // 1. Microtarefas, todas, antes de qualquer timer.
        let imediata = EVENTOS.with(|e| e.borrow_mut().imediatas.pop_front());
        if let Some((raiz, closure)) = imediata {
            // A raiz só sai depois da chamada: o valor da closure é argumento
            // do código gerado, que o enraíza no quadro dele.
            chamar(closure);
            soltar_raiz(raiz);
            continue;
        }
        // 2. O timer de menor (prazo, id); dorme até o prazo se preciso.
        let proximo = EVENTOS.with(|e| {
            let mut e = e.borrow_mut();
            loop {
                let (&(prazo, seq), &id) = e.fila.iter().next()?;
                e.fila.remove(&(prazo, seq));
                if e.ativos.contains_key(&id) {
                    return Some((prazo, id));
                }
                // Cancelado: a entrada da fila é descartada.
            }
        });
        let Some((prazo, id)) = proximo else {
            return;
        };
        let espera = EVENTOS.with(|e| prazo - e.borrow_mut().agora());
        if espera > 0 {
            std::thread::sleep(std::time::Duration::from_millis(espera as u64));
        }
        let disparo = EVENTOS.with(|e| {
            let mut e = e.borrow_mut();
            let t = e.ativos.get(&id)?;
            let (closure, periodico) = (t.closure, t.periodico);
            if periodico {
                return Some((closure, true, None));
            }
            let t = e.ativos.remove(&id).expect("timer ativo");
            Some((closure, false, Some(t.raiz)))
        });
        let Some((closure, periodico, raiz)) = disparo else {
            continue;
        };
        chamar(closure);
        if let Some(r) = raiz {
            soltar_raiz(r);
        }
        if periodico {
            // Como a VM (`_runTimers`): reagendado DEPOIS do callback, com
            // sequência nova, se o callback não o cancelou.
            EVENTOS.with(|e| {
                let mut e = e.borrow_mut();
                let Some(ms) = e.ativos.get(&id).map(|t| t.ms) else { return };
                let novo = if ms > 0 { prazo + ms } else { e.agora() };
                e.prox_seq += 1;
                let seq = e.prox_seq;
                e.fila.insert((novo, seq), id);
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
pub extern "C" fn dartforge_nativo_Error_trySetStackTrace(_erro: i64, _rastro: i64) {}
