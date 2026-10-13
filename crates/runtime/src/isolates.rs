// Native runtime: isolates (`Isolate.spawn`, `Isolate.exit`, the control port) —
// what the VM does in `runtime/vm/isolate.cc` and `runtime/lib/isolate.cc`.
//
// An isolate is a thread with its own heap, its own globals area and its own
// event loop: all runtime state is per thread. Generated code hands over at
// entry the preparation of an isolate (`df.prepare_isolate`: the library
// registrations, the RTI and the embedder) and the function that calls a
// closure; `Isolate.spawn` copies the entry and the message into graphs
// (`ports.rs`), creates the thread, and on it prepares the isolate, sends the
// ready message (`[control port, [pause, termination]]`) and runs the entry as
// the first message (`_startIsolate`). Canonical constants and top-level
// tear-offs cross by identity (the destination receives its own), as in the VM's
// isolate group.
//
// The control port receives the VM's OOB messages (`pause`, `resume`, `ping`,
// `kill`, exit and error listeners, fatal errors), which the event loop serves
// between one event and the next, before the ordinary ones. When it ends, the
// isolate notifies the exit listeners and closes its ports. An unhandled error
// goes to the error listeners (`[error, trace]`); without a listener and with
// fatal errors, it is printed as in the VM (`Unhandled exception:`) and the
// isolate ends — the process does not: it ends with the main isolate.
//
// `Isolate.exit` and `kill` unwind the isolate (the VM's `UnwindError`): the
// pending exception cannot be caught (`exceptions.rs`). Control is served between
// events and also at the safepoint of each loop iteration
// ([`dartforge_safepoint`], the role of the VM's stack check): an immediate
// `kill` or a `ping` reach an isolate stuck in a loop without events; a `kill`
// with `beforeNextEvent` waits for the current event to finish.

/// `Isolate.immediate` (the priority of `kill` and `ping`).
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

/// The stack of each isolate thread (a reservation; the system only uses what is
/// touched).
const ISOLATE_STACK: usize = 64 * 1024 * 1024;

/// What generated code hands over at entry: the preparation of an isolate and the
/// closure call (0 = not registered yet).
static PREPARE_ISOLATE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CHAMAR_CLOSURE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// `dartforge_register_isolates(prepare, call)`: the program entry hands over the
/// isolate preparation and the closure call.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_register_isolates(prepare: extern "C" fn(), call: Option<extern "C" fn(i64) -> i64>) {
    PREPARE_ISOLATE.store(prepare as usize, std::sync::atomic::Ordering::Release);
    CHAMAR_CLOSURE.store(call.map_or(0, |c| c as usize), std::sync::atomic::Ordering::Release);
}

/// The control state of an isolate.
struct IsolateState {
    control: i64,
    pause: i64,
    termination: i64,
    errors_fatal: bool,
    /// The resume capabilities of the requested pauses (paused if there are any).
    pauses: Vec<i64>,
    /// (port, response) of each exit listener.
    exit_listeners: Vec<(i64, Graph)>,
    error_listeners: Vec<i64>,
    /// A `kill` was accepted: the isolate ends on its next return to the loop.
    shut_down: bool,
    /// The accepted `kill` is immediate (`Isolate.immediate`): a loop's safepoint
    /// unwinds the isolate without waiting for the event loop (J01).
    shut_down_now: bool,
}

thread_local! {
    static ISOLATE: RefCell<Option<IsolateState>> = const { RefCell::new(None) };
}

/// Control port → debug name (`Isolate.debugName`), of all live isolates.
fn isolate_names() -> std::sync::MutexGuard<'static, crate::hash::HashMap<i64, String>> {
    static N: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<i64, String>>> = std::sync::OnceLock::new();
    N.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// The state of this isolate, created the first time (the main one is born this
/// way: fatal errors, no pause, no listeners).
fn with_state<R>(f: impl FnOnce(&mut IsolateState) -> R) -> R {
    ISOLATE.with(|i| {
        let mut i = i.borrow_mut();
        let e = i.get_or_insert_with(|| {
            let control = open_control_port();
            isolate_names().insert(control, "main".to_string());
            IsolateState {
                control,
                pause: dartforge_native_DartForge_capability_new(),
                termination: dartforge_native_DartForge_capability_new(),
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

/// Whether the isolate is paused (the loop only serves the control port).
fn paused_isolate() -> bool {
    ISOLATE.with(|i| i.borrow().as_ref().is_some_and(|e| !e.pauses.is_empty()))
}

/// The integer of a Dart value (an unboxed scalar, or a `Ref` to an `int`).
fn int_from(v: crate::heap::Value) -> Option<i64> {
    match v {
        crate::heap::Value::Int(i) => Some(i),
        crate::heap::Value::Ref(r) => HEAP.with(|h| h.borrow().int_of(r)),
        _ => None,
    }
}

/// The `_id` of a `_SendPort` or `_Capability` (the first field: raw, or a `Ref`
/// to an `int`).
fn object_id(h: i64) -> Option<i64> {
    if !crate::layout::is_object(h) {
        return None;
    }
    let (bits, is_ref_kind) = HEAP.with(|heap| heap.borrow().object(h).and_then(|o| o.first()))?;
    if is_ref_kind { int_from(crate::heap::Value::Ref(bits)) } else { Some(bits) }
}

/// Calls function `name` of the `dart:isolate` overlay.
fn ajudante_de_isolado(name: &str) -> usize {
    ajudante(name).unwrap_or_else(|| panic!("compiler bug: dart:isolate without a registered `{name}`"))
}

/// Copies `value` to another isolate; `None` if it is not sendable (with the
/// description in `DartForge_port_refusal`).
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
// The control port.

/// Serves the pending control messages. Returns `false` if the isolate must end
/// (`kill`).
fn serve_control() -> bool {
    while let Some(g) = next_control() {
        with_control_message(&g, handle_control_message);
    }
    !ISOLATE.with(|i| i.borrow().as_ref().is_some_and(|e| e.shut_down))
}

/// Keeps the owner of the materialized message until handling ends. Publication in
/// the slot happens before any allocation or Dart call.
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
        let input = Portable::Str("input error".into()).to_graph();
        let message = Portable::Str("discarded argument".into()).to_graph();
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
        let input = Portable::Str("input".into()).to_graph();
        let message = Portable::Str("argumento".into()).to_graph();
        let (input, message, ready_flag) = with_isolate_entry(&input, &message, |input, message, frame| {
            let ready_flag = HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert_eq!(h.text(input).unwrap().para_string(), "input");
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
            h.allocate_str("protected error")
        });
        dartforge_exception_throw(error, 3);
        let trace = with_unhandled_error(|captured_value, trace, frame| {
            assert_eq!(captured_value, error);
            assert_eq!(dartforge_exception_pending(), 0);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert_eq!(h.text(error).unwrap().para_string(), "protected error");
                assert!(h.is_live_object(trace));
                // The diagnostic may repeat the error: it is another owner occurrence.
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
            let text = h.allocate_str("portable control");
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
                assert_eq!(h.text(msg).unwrap().para_string(), "portable control");
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

/// `IsolateMessageHandler::HandleLibMessage`: `[0, type, …]`.
fn handle_control_message(msg: i64) {
    use crate::heap::Value;
    // The message is a core list (`lists.rs`): the elements unboxed, in the
    // storage's form.
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
    // The element in a `Ref` position (boxes the scalar of a compact list).
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
            // Both priorities answer here: control is served between events,
            // before the next one.
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
// Unhandled errors and the end of the isolate.

/// `ProcessUnhandledException`: the pending exception goes to the error listeners
/// (`[error, trace]`); without a listener and with fatal errors, it is printed.
/// Returns whether the isolate must end.
fn relatar_erro_nao_tratado() -> bool {
    with_unhandled_error(report_error_guarded)
}

/// Copies the owners before clearing the pending exception and keeps the
/// diagnostic in slot 2. Handling may allocate or return early; all slots leave on
/// return.
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

/// Describes and distributes the error already protected by the handling frame.
fn report_error_guarded(error: i64, trace: i64, frame: i64) -> bool {
    let (fatal, listeners) = with_state(|e| (e.errors_fatal, e.error_listeners.clone()));
    // SAFETY: registered by the `dart:isolate` overlay with the signature
    // (`Object?`, `Object?`) → `List<String>`.
    let describe = { let dart_target: usize = ajudante_de_isolado("_dartforgeDescreverErro"); move |a0: i64, a1: i64| -> i64 { dart_r2(dart_target, a0, a1) } };
    let description = describe(error, trace);
    if dartforge_exception_pending() != 0 {
        // The error's `toString()` failed: the runtime's description.
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

/// The end of an isolate: the exit listeners receive their response and the ports
/// close.
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

/// The event loop of an isolate with the handling of unhandled errors: a non-fatal
/// error does not end it.
fn run_isolate_loop(call: extern "C" fn(i64) -> i64) {
    loop {
        dartforge_event_loop(call);
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

/// The safepoint of a loop iteration (J01), which generated code calls when the
/// control port has a message (`Context::interrupt`): serves control (ping, pause,
/// listeners, `kill`) and, with an immediate `kill`, unwinds the isolate — the
/// uncatchable pending exception goes up to the event loop, and the `finally`
/// blocks on the way run. The publication of a JIT reload is not served here: it
/// needs a stack without Dart frames.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_safepoint() {
    if unwinding_now() {
        return;
    }
    serve_control();
    if ISOLATE.with(|i| i.borrow().as_ref().is_some_and(|e| e.shut_down_now)) {
        begin_unwind();
    }
}

/// The interrupt request without the context in generated code (the slow path of
/// [`dartforge_safepoint`]; the emitter reads the byte directly).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_pending_interrupt() -> u8 {
    crate::heap::CONTEXTO.with(|c| c.interrupt.load(std::sync::atomic::Ordering::Acquire))
}

// ---------------------------------------------------------------------------
// `Isolate.spawn`.

/// What the new thread receives from the isolate that created it.
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

/// Materializes the entry and the argument keeping owners until the call returns.
/// The third slot belongs to the ready message produced by the handling.
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

/// The life of an isolate created by `Isolate.spawn`, on its thread.
fn rodar_isolado(p: IsolateRequest) {
    let prepare = PREPARE_ISOLATE.load(std::sync::atomic::Ordering::Acquire);
    let call = CHAMAR_CLOSURE.load(std::sync::atomic::Ordering::Acquire);
    // SAFETY: registered by `dartforge_register_isolates` with these signatures,
    // before any `Isolate.spawn`.
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
    // The isolate's entry and loop in a native scope (prepared ARC images): the
    // owners of the Dart ports called by the runtime.
    in_native_scope_if_arc(|| run_entry_and_loop(&p, call, control, pause, termination));
    shut_down_isolate();
}

fn run_entry_and_loop(p: &IsolateRequest, call: extern "C" fn(i64) -> i64, control: i64, pause: i64, termination: i64) {
    with_isolate_entry(&p.input, &p.message, |input, message, frame| {
        // SAFETY: registered by the `dart:isolate` overlay with the signatures
        // (`int`, `int`, `int`) → `List` and (`Function`, `Object?`) → `void`.
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
pub extern "C" fn dartforge_native_Isolate_spawnFunction(
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
        throw_internal_error("Isolate.spawn without the program entry registered");
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
        // The allocator's free lists are valid while the isolate runs
        // (`allocator.rs`); the end of the thread returns them to the system.
        enable_allocation_cache();
        rodar_isolado(request);
        flush_allocation_cache();
    });
    if let Err(e) = created {
        // The VM answers on the ready port with the error text.
        post(ready_flag, Portable::Str(format!("Unable to create the isolate thread: {e}")).to_graph());
    }
}

/// `Isolate_spawnUri`: neither the native executable nor the JIT loads another
/// program (the VM's AOT runtime only accepts an AOT snapshot, and with a `.dart`
/// it fails too); the ready port receives the error.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn dartforge_native_Isolate_spawnUri(
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
    // The JIT (the host set the script) runs one program per process: the class
    // tables and the RTI belong to the process. See docs/JIT.md.
    let jit = program_script().lock().unwrap_or_else(|e| e.into_inner()).is_some();
    let text = if jit {
        format!("Isolate.spawnUri({uri}) is not supported by the DartForge JIT: a process runs a single program")
    } else {
        format!("Isolate.spawnUri({uri}) is not supported in a program compiled ahead of time")
    };
    post(ready_flag, Portable::Str(text).to_graph());
}

/// `Isolate_getPortAndCapabilitiesOfCurrentIsolate`: `[control port, pause,
/// termination]`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Isolate_getPortAndCapabilitiesOfCurrentIsolate() -> i64 {
    let (c, p, t) = with_state(|e| (e.control, e.pause, e.termination));
    // SAFETY: registered by the `dart:isolate` overlay with the signature (`int`,
    // `int`, `int`) → `List`.
    let f = { let dart_target: usize = ajudante_de_isolado("_dartforgePortaECapacidades"); move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(dart_target, a0, a1, a2) } };
    f(c, p, t)
}

/// `Isolate_getDebugName(controlPort)`: the name of the live isolate, or `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Isolate_getDebugName(port: i64) -> i64 {
    let Some(id) = object_id(port) else { return 0 };
    let name = isolate_names().get(&id).cloned();
    name.map_or(0, |n| HEAP.with(|h| h.borrow_mut().allocate_str(&n)))
}

/// `Isolate_getCurrentRootUriStr`: the `file:` URI of the script (the `.dart` the
/// JIT runs, as in `dart run`; in AOT, the executable, as in the VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Isolate_getCurrentRootUriStr() -> i64 {
    let defined = program_script().lock().unwrap_or_else(|e| e.into_inner()).clone();
    let file_path = match defined {
        Some(b) => path_from_bytes(&b),
        None => std::env::current_exe().ok().or_else(|| std::env::args_os().next().map(std::path::PathBuf::from)).unwrap_or_default(),
    };
    let absolute_path = if file_path.is_absolute() { file_path } else { std::env::current_dir().unwrap_or_default().join(file_path) };
    HEAP.with(|h| h.borrow_mut().allocate_str(&file_uri(&absolute_path)))
}

/// The `file:` URI of an absolute path (`Uri.file`), with the bytes outside the
/// unreserved set percent-encoded.
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

/// `Isolate_sendOOB(port, msg)`: the message goes to the control queue of the
/// isolate that owns the port.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Isolate_sendOOB(port: i64, message: i64) {
    let Some(id) = object_id(port) else {
        throw_argument_error("port is not a SendPort");
        return;
    };
    let Some(g) = copy_to_other_isolate(message) else { return };
    post_control(id, g);
}

/// `Isolate_exit_(finalMessagePort, message)`: sends the final message and unwinds
/// the isolate (the VM's `UnwindError`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Isolate_exit_(port: i64, message: i64) -> i64 {
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
