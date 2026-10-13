// Runtime nativo: os isolados (`Isolate.spawn`, `Isolate.exit`, a porta de
// controle) — o que a VM faz em `runtime/vm/isolate.cc` e
// `runtime/lib/isolate.cc`.
//
// Um isolado é uma thread com o próprio heap, a própria área de globais e o
// próprio laço de eventos: todo o estado do runtime é por thread. O código
// gerado entrega na entrada a preparação de um isolado
// (`df.preparar_isolado`: os registros das bibliotecas, a RTI e o
// embedder) e a função que chama uma closure; `Isolate.spawn` copia a
// entrada e a mensagem para grafos (`portas.rs`), cria a thread, e nela
// prepara o isolado, manda a mensagem de pronto (`[porta de controle,
// [pausa, término]]`) e roda a entrada como a primeira mensagem
// (`_startIsolate`). Constantes canônicas e tear-offs de topo atravessam
// pela identidade (o destino recebe as dele), como no grupo de isolados da
// VM.
//
// A porta de controle recebe as mensagens OOB da VM (`pause`, `resume`,
// `ping`, `kill`, ouvintes de saída e de erro, erros fatais), que o laço de
// eventos atende entre um evento e outro, antes dos comuns. Ao terminar, o
// isolado avisa os ouvintes de saída e fecha as portas. Um erro não tratado
// vai aos ouvintes de erro (`[error, trace]`); sem ouvinte e com erros
// fatais, é impresso como na VM (`Unhandled exception:`) e o isolado
// termina — o processo, não: ele termina com o isolado principal.
//
// `Isolate.exit` e o `kill` desenrolam o isolado (o `UnwindError` da VM):
// a exceção pendente não é capturável (`excecoes.rs`). O controle é atendido
// entre eventos e também no ponto seguro de cada volta de laço
// ([`dartforge_ponto_seguro`], o papel da verificação de pilha da VM): um
// `kill` imediato ou um `ping` alcançam um isolado preso num laço sem
// eventos; o `kill` com `beforeNextEvent` espera o evento atual terminar.

/// `Isolate.immediate` (a prioridade de `kill` e `ping`).
const IMMEDIATE_PRIORITY: i64 = 0;

/// `IsolateMessageHandler` (`isolate_patch.dart`, `_PAUSE`…).
const OOB_PAUSE: i64 = 1;
const OOB_RESUME: i64 = 2;
const OOB_PING: i64 = 3;
const OOB_KILL: i64 = 4;
const OOB_LISTEN_EXIT: i64 = 5;
const OOB_UNLISTEN_EXIT: i64 = 6;
const OOB_LISTEN_ERROR: i64 = 7;
const OOB_UNLISTEN_ERROR: i64 = 8;
const OOB_ERRORS_FATAL: i64 = 9;

/// A pilha de cada thread de isolado (reserva; o sistema só usa o que
/// tocar).
const ISOLATE_STACK: usize = 64 * 1024 * 1024;

/// O que o código gerado entrega na entrada: a preparação de um isolado e
/// a chamada de closure (0 = ainda não registrado).
static PREPARE_ISOLATE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CHAMAR_CLOSURE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// `dartforge_registrar_isolados(prepare, call)`: a entrada do programa
/// entrega a preparação de isolado e a chamada de closure.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_registrar_isolados(prepare: extern "C" fn(), call: Option<extern "C" fn(i64) -> i64>) {
    PREPARE_ISOLATE.store(prepare as usize, std::sync::atomic::Ordering::Release);
    CHAMAR_CLOSURE.store(call.map_or(0, |c| c as usize), std::sync::atomic::Ordering::Release);
}

/// O estado de controle de um isolado.
struct IsolateState {
    control: i64,
    pause: i64,
    termination: i64,
    errors_fatal: bool,
    /// As capacidades de retomada das pausas pedidas (pausado se houver).
    pauses: Vec<i64>,
    /// (porta, resposta) de cada ouvinte de saída.
    exit_listeners: Vec<(i64, Graph)>,
    error_listeners: Vec<i64>,
    /// Um `kill` foi aceito: o isolado termina no próximo retorno ao laço.
    shut_down: bool,
    /// O `kill` aceito é imediato (`Isolate.immediate`): o ponto seguro de
    /// um laço desenrola o isolado sem esperar o laço de eventos (J01).
    shut_down_now: bool,
}

thread_local! {
    static ISOLATE: RefCell<Option<IsolateState>> = const { RefCell::new(None) };
}

/// Porta de controle → nome de depuração (`Isolate.debugName`), de todos os
/// isolados vivos.
fn isolate_names() -> std::sync::MutexGuard<'static, crate::hash::HashMap<i64, String>> {
    static N: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<i64, String>>> = std::sync::OnceLock::new();
    N.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// O estado deste isolado, criado na primeira vez (o principal nasce
/// assim: erros fatais, sem pausa, sem ouvintes).
fn with_state<R>(f: impl FnOnce(&mut IsolateState) -> R) -> R {
    ISOLATE.with(|i| {
        let mut i = i.borrow_mut();
        let e = i.get_or_insert_with(|| {
            let control = open_control_port();
            isolate_names().insert(control, "main".to_string());
            IsolateState {
                control,
                pause: dartforge_nativo_DartForge_capacidade_nova(),
                termination: dartforge_nativo_DartForge_capacidade_nova(),
                errors_fatal: true,
                pauses: Vec::new(),
                exit_listeners: Vec::new(),
                error_listeners: Vec::new(),
                shut_down: false,
                shut_down_now: false,
            }
        });
        f(e)
    })
}

/// Se o isolado está pausado (o laço só atende a porta de controle).
fn paused_isolate() -> bool {
    ISOLATE.with(|i| i.borrow().as_ref().is_some_and(|e| !e.pauses.is_empty()))
}

/// O inteiro de um valor Dart (escalar sem caixa, ou `Ref` de `int`).
fn int_from(v: crate::heap::Value) -> Option<i64> {
    match v {
        crate::heap::Value::Int(i) => Some(i),
        crate::heap::Value::Ref(r) => HEAP.with(|h| h.borrow().int_of(r)),
        _ => None,
    }
}

/// O `_id` de uma `_SendPort` ou `_Capability` (o primeiro campo: bruto, ou
/// um `Ref` de `int`).
fn object_id(h: i64) -> Option<i64> {
    if !crate::layout::is_object(h) {
        return None;
    }
    let (bits, is_ref_kind) = HEAP.with(|heap| heap.borrow().object(h).and_then(|o| o.first()))?;
    if is_ref_kind { int_from(crate::heap::Value::Ref(bits)) } else { Some(bits) }
}

/// Chama a função `name` da sobreposição de `dart:isolate`.
fn ajudante_de_isolado(name: &str) -> usize {
    ajudante(name).unwrap_or_else(|| panic!("bug do compilador: dart:isolate sem `{name}` registrado"))
}

/// Copia `value` para outro isolado; `None` se não é enviável (com a
/// descrição em `DartForge_porta_recusa`).
fn copy_to_other_isolate(value: i64) -> Option<Graph> {
    match copy_to_graph(value, false) {
        Ok(g) => Some(g),
        Err(IllegalMessage(d)) => {
            throw_argument_error(&format!("Illegal argument in isolate message: {d}"));
            None
        }
    }
}

// ---------------------------------------------------------------------------
// A porta de controle.

/// Atende as mensagens de controle pendentes. Devolve `false` se o isolado
/// deve terminar (`kill`).
fn serve_control() -> bool {
    while let Some(g) = next_control() {
        with_control_message(&g, handle_control_message);
    }
    !ISOLATE.with(|i| i.borrow().as_ref().is_some_and(|e| e.shut_down))
}

/// Mantém o owner da mensagem materializada até o tratamento terminar.
/// A publicação no slot ocorre antes de qualquer alocação ou chamada Dart.
fn with_control_message<R>(graph: &Graph, handle_it: impl FnOnce(i64) -> R) -> R {
    let msg = materialize(graph);
    let frame = HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let frame = h.push_owning_frame(1);
        h.set_root(frame, 0, msg);
        frame
    });
    let result = handle_it(msg);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    result
}

#[cfg(test)]
mod tests_control_owner {
    use super::*;

    #[test]
    fn entry_with_exception_releases_argument_and_keeps_pending_owner() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        HEAP.with(|h| h.borrow_mut().enable_arc());
        let input = Portable::Str("erro da entrada".into()).to_graph();
        let message = Portable::Str("argumento descartado".into()).to_graph();
        let (input, message) = with_isolate_entry(&input, &message, |input, message, _| {
            dartforge_exception_throw(input, 3);
            (input, message)
        });
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(h.is_live_object(input));
            assert!(!h.is_live_object(message));
        });
        assert_eq!(dartforge_exception_peek_ref(), input);
        dartforge_exception_clear();
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.is_live_object(input));
        });
        HEAP.with(|h| { h.replace(previous); });
    }

    #[test]
    fn isolate_entry_preserves_values_and_releases_owners_on_return() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        HEAP.with(|h| h.borrow_mut().enable_arc());
        let input = Portable::Str("entrada".into()).to_graph();
        let message = Portable::Str("argumento".into()).to_graph();
        let (input, message, ready_flag) = with_isolate_entry(&input, &message, |input, message, frame| {
            let ready_flag = HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert_eq!(h.text(input).unwrap().para_string(), "entrada");
                assert_eq!(h.text(message).unwrap().para_string(), "argumento");
                let ready_flag = h.allocate_str("pronto");
                h.set_root(frame, 2, ready_flag);
                h.collect();
                assert_eq!(h.text(ready_flag).unwrap().para_string(), "pronto");
                ready_flag
            });
            (input, message, ready_flag)
        });
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            for value in [input, message, ready_flag] {
                assert!(!h.is_live_object(value));
            }
        });
        HEAP.with(|h| { h.replace(previous); });
    }

    #[test]
    fn error_and_trace_survive_clear_until_report_ends() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let error = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            h.allocate_str("erro protegido")
        });
        dartforge_exception_throw(error, 3);
        let trace = with_unhandled_error(|captured_value, trace, frame| {
            assert_eq!(captured_value, error);
            assert_eq!(dartforge_exception_pending(), 0);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert_eq!(h.text(error).unwrap().para_string(), "erro protegido");
                assert!(h.is_live_object(trace));
                // O diagnóstico pode repetir o erro: é outra ocorrência owner.
                h.set_root(frame, 2, error);
                h.collect();
                assert!(h.is_live_object(error));
            });
            trace
        });
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.is_live_object(error));
            assert!(!h.is_live_object(trace));
        });
        HEAP.with(|h| { h.replace(previous); });
    }

    #[test]
    fn portable_copy_survives_in_queue_and_active_owner_ends_at_return() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let (frame, text) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            let frame = h.push_owning_frame(1);
            let text = h.allocate_str("controle portátil");
            h.set_root(frame, 0, text);
            (frame, text)
        });
        let port = open_control_port();
        let graph = copy_to_other_isolate(text).unwrap();
        graph.visit_values(|v| assert!(!matches!(v, GValue::Same(_))));
        post_control(port, graph);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.pop_frame(frame);
            h.collect();
            assert!(!h.is_live_object(text));
        });
        let graph = next_control().unwrap();
        let received = with_control_message(&graph, |msg| {
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert_eq!(h.text(msg).unwrap().para_string(), "controle portátil");
            });
            msg
        });
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.is_live_object(received));
        });
        close_isolate_ports();
        HEAP.with(|h| { h.replace(previous); });
    }
}

/// `IsolateMessageHandler::HandleLibMessage`: `[0, tipo, …]`.
fn handle_control_message(msg: i64) {
    use crate::heap::Value;
    // A mensagem é uma lista do núcleo (`lists.rs`): os elementos sem caixa,
    // na forma do armazenamento.
    let items: Vec<Value> = HEAP.with(|h| {
        let h = h.borrow();
        if !crate::layout::is_object(msg) || !h.e_lista(msg) {
            return Vec::new();
        }
        (0..h.list_len(msg)).map(|i| h.list_get(msg, i)).collect()
    });
    let Some(tipo) = items.get(1).and_then(|v| int_from(*v)) else { return };
    let object = |i: usize| match items.get(i) {
        Some(Value::Ref(r)) => *r,
        _ => 0,
    };
    let id = |i: usize| object_id(object(i));
    // O elemento numa posição `Ref` (encaixota o escalar de uma lista compacta).
    let value = |i: usize| items.get(i).map_or(0, |v| HEAP.with(|h| h.borrow_mut().to_ref(*v)));
    match tipo {
        OOB_PAUSE | OOB_RESUME => {
            let (Some(cap), Some(resume)) = (id(2), id(3)) else { return };
            with_state(|e| {
                if cap != e.pause {
                    return;
                }
                if tipo == OOB_PAUSE {
                    e.pauses.push(resume);
                } else if let Some(k) = e.pauses.iter().position(|&r| r == resume) {
                    e.pauses.remove(k);
                }
            });
        }
        OOB_PING => {
            let Some(port) = id(2) else { return };
            // As duas prioridades respondem aqui: o controle é atendido
            // entre eventos, antes do próximo.
            let response = value(4);
            if let Ok(g) = copy_to_graph(response, false) {
                post(port, g);
            }
        }
        OOB_KILL => {
            let Some(cap) = id(2) else { return };
            let immediate = items.get(3).and_then(|v| int_from(*v)) == Some(IMMEDIATE_PRIORITY);
            with_state(|e| {
                if cap == e.termination {
                    e.shut_down = true;
                    e.shut_down_now |= immediate;
                }
            });
        }
        OOB_LISTEN_EXIT => {
            let Some(port) = id(2) else { return };
            let Ok(response) = copy_to_graph(value(3), false) else { return };
            with_state(|e| {
                e.exit_listeners.retain(|(p, _)| *p != port);
                e.exit_listeners.push((port, response));
            });
        }
        OOB_UNLISTEN_EXIT => {
            let Some(port) = id(2) else { return };
            with_state(|e| e.exit_listeners.retain(|(p, _)| *p != port));
        }
        OOB_LISTEN_ERROR => {
            let Some(port) = id(2) else { return };
            with_state(|e| {
                if !e.error_listeners.contains(&port) {
                    e.error_listeners.push(port);
                }
            });
        }
        OOB_UNLISTEN_ERROR => {
            let Some(port) = id(2) else { return };
            with_state(|e| e.error_listeners.retain(|&p| p != port));
        }
        OOB_ERRORS_FATAL => {
            let Some(cap) = id(2) else { return };
            let fatal = match items.get(3) {
                Some(Value::Bool(b)) => *b,
                Some(Value::Ref(r)) => HEAP.with(|h| h.borrow().bool_of(*r)) == Some(true),
                _ => false,
            };
            with_state(|e| {
                if cap == e.termination {
                    e.errors_fatal = fatal;
                }
            });
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Erros não tratados e o fim do isolado.

/// `ProcessUnhandledException`: a exceção pendente vai aos ouvintes de
/// erro (`[error, trace]`); sem ouvinte e com erros fatais, é impressa.
/// Devolve se o isolado deve terminar.
fn relatar_erro_nao_tratado() -> bool {
    with_unhandled_error(report_error_guarded)
}

/// Copia os owners antes de limpar a pendência e mantém o diagnóstico no slot 2.
/// O tratamento pode alocar ou retornar cedo; todos os slots saem ao retornar.
fn with_unhandled_error<R>(handle_it: impl FnOnce(i64, i64, i64) -> R) -> R {
    let error = dartforge_exception_peek_ref();
    let trace = dartforge_stack_trace_get();
    let frame = HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let frame = h.push_owning_frame(3);
        h.set_root(frame, 0, error);
        h.set_root(frame, 1, trace);
        frame
    });
    dartforge_exception_clear();
    let result = handle_it(error, trace, frame);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    result
}

/// Descreve e distribui o erro já protegido pelo quadro do tratamento.
fn report_error_guarded(error: i64, trace: i64, frame: i64) -> bool {
    let (fatal, listeners) = with_state(|e| (e.errors_fatal, e.error_listeners.clone()));
    // SAFETY: registrada pela sobreposição de `dart:isolate` com a
    // assinatura (`Object?`, `Object?`) → `List<String>`.
    let describe = { let dart_target: usize = ajudante_de_isolado("_dartforgeDescreverErro"); move |a0: i64, a1: i64| -> i64 { dart_r2(dart_target, a0, a1) } };
    let description = describe(error, trace);
    if dartforge_exception_pending() != 0 {
        // O `toString()` do erro falhou: a descrição do runtime.
        dartforge_exception_clear();
        if listeners.is_empty() && fatal {
            eprintln!("Unhandled exception:\n{}", HEAP.with(|h| describe_handle(&h.borrow(), error)));
        }
        return fatal;
    }
    HEAP.with(|h| h.borrow_mut().set_root(frame, 2, description));
    if listeners.is_empty() {
        if fatal {
            let textos: Vec<String> = HEAP.with(|h| {
                let h = h.borrow();
                if !crate::layout::is_object(description) || !h.e_lista(description) {
                    return Vec::new();
                }
                (0..h.list_len(description))
                    .map(|i| match h.list_get(description, i) {
                        crate::heap::Value::Ref(r) => h.text(r).map(|t| t.para_string()).unwrap_or_default(),
                        _ => String::new(),
                    })
                    .collect()
            });
            let trace = textos.get(1).map(String::as_str).unwrap_or("");
            eprintln!("Unhandled exception:\n{}\n{}", textos.first().map(String::as_str).unwrap_or(""), trace.trim_end());
        }
    } else if let Ok(g) = copy_to_graph(description, false) {
        for (k, port) in listeners.iter().enumerate() {
            if k + 1 == listeners.len() {
                post(*port, g);
                break;
            }
            post(*port, g.clone());
        }
    }
    fatal
}

/// O fim de um isolado: os ouvintes de saída recebem a resposta deles e as
/// portas fecham.
fn shut_down_isolate() {
    let (control, listeners) = with_state(|e| (e.control, std::mem::take(&mut e.exit_listeners)));
    for (port, response) in listeners {
        post(port, response);
    }
    isolate_names().remove(&control);
    shutdown_isolate_finalizers();
    remove_live_isolate();
    close_isolate_ports();
}

/// O laço de eventos de um isolado com o tratamento dos erros não
/// tratados: um erro não fatal não o termina.
fn run_isolate_loop(call: extern "C" fn(i64) -> i64) {
    loop {
        dartforge_laco_de_eventos(call);
        if unwinding_now() || ISOLATE.with(|i| i.borrow().as_ref().is_some_and(|e| e.shut_down)) {
            return;
        }
        if dartforge_exception_pending() == 0 {
            return;
        }
        if relatar_erro_nao_tratado() {
            return;
        }
    }
}

/// O ponto seguro de uma volta de laço (J01), que o código gerado chama
/// quando a porta de controle tem mensagem (`Context::interrupt`): atende
/// o controle (ping, pausa, ouvintes, `kill`) e, com um `kill` imediato,
/// desenrola o isolado — a exceção pendente não capturável sobe até o laço de
/// eventos, e os `finally` do caminho rodam. A publicação de uma recarga do
/// JIT não é atendida aqui: ela precisa da pilha sem quadros Dart.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ponto_seguro() {
    if unwinding_now() {
        return;
    }
    serve_control();
    if ISOLATE.with(|i| i.borrow().as_ref().is_some_and(|e| e.shut_down_now)) {
        begin_unwind();
    }
}

/// O pedido de interrupção sem o contexto no código gerado (o caminho lento
/// de [`dartforge_ponto_seguro`]; o emissor lê o byte direto).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_interrupcao_pendente() -> u8 {
    crate::heap::CONTEXTO.with(|c| c.interrupt.load(std::sync::atomic::Ordering::Acquire))
}

// ---------------------------------------------------------------------------
// `Isolate.spawn`.

/// O que a thread nova recebe do isolado que a criou.
struct IsolateRequest {
    ready_flag: i64,
    input: Graph,
    message: Graph,
    paused: bool,
    errors_fatal: bool,
    exit_listener: Option<i64>,
    error_listener: Option<i64>,
    name: String,
}

/// Materializa entrada e argumento mantendo owners até o retorno da chamada.
/// O terceiro slot pertence à mensagem de pronto produzida pelo tratamento.
fn with_isolate_entry<R>(input: &Graph, message: &Graph, handle_it: impl FnOnce(i64, i64, i64) -> R) -> R {
    let frame = HEAP.with(|h| h.borrow_mut().push_owning_frame(3));
    let input = materialize(input);
    HEAP.with(|h| h.borrow_mut().set_root(frame, 0, input));
    let message = materialize(message);
    HEAP.with(|h| h.borrow_mut().set_root(frame, 1, message));
    let result = handle_it(input, message, frame);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    result
}

/// A vida de um isolado criado por `Isolate.spawn`, na thread dele.
fn rodar_isolado(p: IsolateRequest) {
    let prepare = PREPARE_ISOLATE.load(std::sync::atomic::Ordering::Acquire);
    let call = CHAMAR_CLOSURE.load(std::sync::atomic::Ordering::Acquire);
    // SAFETY: registrados por `dartforge_registrar_isolados` com essas
    // assinaturas, antes de qualquer `Isolate.spawn`.
    let prepare = { let dart_target: usize = prepare; move || { dart_v0(dart_target) } };
    let call: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(call) };
    prepare();
    let (control, pause, termination) = with_state(|e| {
        e.errors_fatal = p.errors_fatal;
        if p.paused {
            e.pauses.push(e.pause);
        }
        if let Some(port) = p.exit_listener {
            e.exit_listeners.push((port, Graph::null_val()));
        }
        e.error_listeners.extend(p.error_listener);
        (e.control, e.pause, e.termination)
    });
    isolate_names().insert(control, p.name.clone());
    if dartforge_exception_pending() != 0 {
        relatar_erro_nao_tratado();
        shut_down_isolate();
        return;
    }
    // A entrada e o laço do isolado num escopo nativo (imagens ARC
    // preparadas): os owners das portas Dart chamadas pelo runtime.
    in_native_scope_if_arc(|| run_entry_and_loop(&p, call, control, pause, termination));
    shut_down_isolate();
}

fn run_entry_and_loop(p: &IsolateRequest, call: extern "C" fn(i64) -> i64, control: i64, pause: i64, termination: i64) {
    with_isolate_entry(&p.input, &p.message, |input, message, frame| {
        // SAFETY: registradas pela sobreposição de `dart:isolate` com as
        // assinaturas (`int`, `int`, `int`) → `List` e (`Function`,
        // `Object?`) → `void`.
        let ready_flag = { let dart_target: usize = ajudante_de_isolado("_dartforgeMensagemDePronto"); move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(dart_target, a0, a1, a2) } };
        let iniciar = { let dart_target: usize = ajudante_de_isolado("_dartforgeIniciarIsolado"); move |a0: i64, a1: i64| { dart_v2(dart_target, a0, a1) } };
        let m = ready_flag(control, pause, termination);
        if dartforge_exception_pending() != 0 { return; }
        HEAP.with(|h| h.borrow_mut().set_root(frame, 2, m));
        if let Ok(g) = copy_to_graph(m, false) {
            post(p.ready_flag, g);
        }
        iniciar(input, message);
    });
    if dartforge_exception_pending() == 0 || !relatar_erro_nao_tratado() {
        run_isolate_loop(call);
    }
}

/// `Isolate_spawnFunction(readyPort, uri, entryPoint, message, paused,
/// errorsAreFatal, onExit, onError, packageConfig, debugName)`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn dartforge_nativo_Isolate_spawnFunction(
    ready_flag: i64,
    uri: i64,
    input: i64,
    message: i64,
    paused: u8,
    errors_fatal: u8,
    exit_listener: i64,
    error_listener: i64,
    _package_config: i64,
    name: i64,
) {
    let Some(ready_flag) = object_id(ready_flag) else {
        throw_argument_error("readyPort is not a SendPort");
        return;
    };
    if PREPARE_ISOLATE.load(std::sync::atomic::Ordering::Acquire) == 0 || CHAMAR_CLOSURE.load(std::sync::atomic::Ordering::Acquire) == 0 {
        throw_internal_error("Isolate.spawn sem a entrada do programa registrada");
        return;
    }
    let Some(g_input) = copy_to_other_isolate(input) else { return };
    let Some(g_message) = copy_to_other_isolate(message) else { return };
    let text = |h: i64| HEAP.with(|heap| heap.borrow().text(h).map(|t| t.para_string()));
    let name = text(name).unwrap_or_else(|| format!("{}:spawn()", text(uri).unwrap_or_default()));
    let request = IsolateRequest {
        ready_flag,
        input: g_input,
        message: g_message,
        paused: paused != 0,
        errors_fatal: errors_fatal != 0,
        exit_listener: (exit_listener != 0).then(|| object_id(exit_listener)).flatten(),
        error_listener: (error_listener != 0).then(|| object_id(error_listener)).flatten(),
        name: name.clone(),
    };
    let created = std::thread::Builder::new().name(name).stack_size(ISOLATE_STACK).spawn(move || {
        // As listas livres do alocador valem enquanto o isolado roda
        // (`alocador.rs`); o fim da thread as devolve ao sistema.
        enable_allocation_cache();
        rodar_isolado(request);
        flush_allocation_cache();
    });
    if let Err(e) = created {
        // A VM responde na porta de pronto com o texto do erro.
        post(ready_flag, Portable::Str(format!("Unable to create the isolate thread: {e}")).to_graph());
    }
}

/// `Isolate_spawnUri`: nem o executável nativo nem o JIT carregam outro
/// programa (o runtime AOT da VM só aceita um snapshot AOT, e com um `.dart`
/// também falha); a porta de pronto recebe o erro.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn dartforge_nativo_Isolate_spawnUri(
    ready_flag: i64,
    uri: i64,
    _args: i64,
    _message: i64,
    _paused: u8,
    _out: i64,
    _error: i64,
    _fatal: u8,
    _asserts: i64,
    _env: i64,
    _packages: i64,
    _name: i64,
) {
    let Some(ready_flag) = object_id(ready_flag) else { return };
    let uri = HEAP.with(|h| h.borrow().text(uri).map(|t| t.para_string()).unwrap_or_default());
    // O JIT (o hospedeiro definiu o script) roda um programa por processo:
    // as tabelas de classes e o RTI são do processo. Ver docs/JIT.md.
    let jit = program_script().lock().unwrap_or_else(|e| e.into_inner()).is_some();
    let text = if jit {
        format!("Isolate.spawnUri({uri}) is not supported by the DartForge JIT: a process runs a single program")
    } else {
        format!("Isolate.spawnUri({uri}) is not supported in a program compiled ahead of time")
    };
    post(ready_flag, Portable::Str(text).to_graph());
}

/// `Isolate_getPortAndCapabilitiesOfCurrentIsolate`: `[porta de controle,
/// pausa, término]`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Isolate_getPortAndCapabilitiesOfCurrentIsolate() -> i64 {
    let (c, p, t) = with_state(|e| (e.control, e.pause, e.termination));
    // SAFETY: registrada pela sobreposição de `dart:isolate` com a
    // assinatura (`int`, `int`, `int`) → `List`.
    let f = { let dart_target: usize = ajudante_de_isolado("_dartforgePortaECapacidades"); move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(dart_target, a0, a1, a2) } };
    f(c, p, t)
}

/// `Isolate_getDebugName(controlPort)`: o nome do isolado vivo, ou `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Isolate_getDebugName(port: i64) -> i64 {
    let Some(id) = object_id(port) else { return 0 };
    let name = isolate_names().get(&id).cloned();
    name.map_or(0, |n| HEAP.with(|h| h.borrow_mut().allocate_str(&n)))
}

/// `Isolate_getCurrentRootUriStr`: o URI `file:` do script (o `.dart` que
/// o JIT roda, como no `dart run`; no AOT, o executável, como na VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Isolate_getCurrentRootUriStr() -> i64 {
    let defined = program_script().lock().unwrap_or_else(|e| e.into_inner()).clone();
    let file_path = match defined {
        Some(b) => path_from_bytes(&b),
        None => std::env::current_exe().ok().or_else(|| std::env::args_os().next().map(std::path::PathBuf::from)).unwrap_or_default(),
    };
    let absolute_path = if file_path.is_absolute() { file_path } else { std::env::current_dir().unwrap_or_default().join(file_path) };
    HEAP.with(|h| h.borrow_mut().allocate_str(&file_uri(&absolute_path)))
}

/// O URI `file:` de um caminho absoluto (`Uri.file`), com os bytes fora dos
/// não reservados codificados.
fn file_uri(file_path: &std::path::Path) -> String {
    let text = file_path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from(if text.starts_with('/') { "file://" } else { "file:///" });
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/:".contains(&b) {
            uri.push(char::from(b));
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    uri
}

/// `Isolate_sendOOB(port, msg)`: a mensagem vai à fila de controle do
/// isolado dono da porta.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Isolate_sendOOB(port: i64, message: i64) {
    let Some(id) = object_id(port) else {
        throw_argument_error("port is not a SendPort");
        return;
    };
    let Some(g) = copy_to_other_isolate(message) else { return };
    post_control(id, g);
}

/// `Isolate_exit_(finalMessagePort, message)`: manda a mensagem final e
/// desenrola o isolado (o `UnwindError` da VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Isolate_exit_(port: i64, message: i64) -> i64 {
    if port != 0 {
        let Some(id) = object_id(port) else {
            throw_argument_error("finalMessagePort is not a SendPort");
            return 0;
        };
        let Some(g) = copy_to_other_isolate(message) else { return 0 };
        post(id, g);
    }
    begin_unwind();
    0
}
