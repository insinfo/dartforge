// Native runtime: the isolate's event loop (P6, docs/NATIVO-PLANO.md §7.7) —
// microtasks and timers.
//
// The microtask queue and its order are those of the source's `dart:async`
// (`schedule_microtask.dart`: `_nextCallback`, `_startMicrotaskLoop`); the
// runtime only keeps the closures that `_AsyncRun._scheduleImmediate` hands over
// (the `DartForge_scheduleImmediate` native of the `native_sdk/` overlay) and
// runs them, in order, before any timer. The timers are the VM's
// (`_internal/vm/lib/timer_impl.dart`, the reference): a deadline in milliseconds
// of a monotonic clock — `current_time` for duration 0 and
// `current_time + 1 + ms` for the others —, ties broken by creation order (the
// VM's `_id`), microtasks drained after each timer, a periodic one rescheduled at
// `deadline + ms`.
//
// It is the ONLY point where the runtime calls Dart code: through the function
// that generated code hands to `dartforge_event_loop` (`call`, which calls a
// closure without arguments through the uniform convention and returns with the
// pending exception, if any). An exception that leaves a microtask or a timer was
// not handled by the `Zone` (the source's `_rootHandleError` rethrows it in an
// `Error.throwWithStackTrace`): the loop stops and `finalize_program` reports
// it, as the VM does with "Unhandled exception".
//
// The stored closures are collector roots while they wait: each one takes a heap
// root global (`set_global_root`) with a NEGATIVE id — the program's globals use
// the non-negative ids.
//
// Per-thread state (one isolate per thread; P8 gathers everything in `isolado.rs`).

struct LoopTimer {
    /// The closure that fires the timer (`_Timer._fire` with the receiver).
    closure: i64,
    root: i64,
    ms: i64,
    periodic: bool,
    tick: i64,
}

#[derive(Default)]
struct Events {
    /// Closures handed over by `DartForge_scheduleImmediate`, with the root.
    immediates: std::collections::VecDeque<(i64, i64)>,
    /// Timers scheduled by `(deadline at ms, sequence)` → id. The sequence is the
    /// VM's `_id`: it grows on every scheduling (and rescheduling).
    queue: std::collections::BTreeMap<(i64, i64), i64>,
    next_seq: i64,
    /// Active timers by id (the one from `DartForge_Timer_new`, never 0).
    active: crate::hash::HashMap<i64, LoopTimer>,
    next_id: i64,
    next_root: i64,
    start: Option<crate::clock::Instant>,
    /// Application-owned native event dispatcher and its persistent heap root.
    native_dispatcher: Option<(i64, i64)>,
}

thread_local! {
    static EVENTS: RefCell<Events> = RefCell::new(Events::default());
    static MICROTASK_DRAIN_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static EVENT_LOOP_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// A WebAssembly module never blocks inside the event loop: the browser's thread
// cannot wait, and the host owns the clock. When only future timers remain,
// the loop records the earliest deadline, marks itself suspended and returns;
// the module's entry reports "pending" (host ABI v1), and the host calls
// `dartforge_resume` (`wasm_entry.rs`) once the delay has passed.
#[cfg(target_arch = "wasm32")]
thread_local! {
    /// The loop returned to the host (`Some`), with the deadline of the next
    /// timer (`None`: a timer too far away to represent).
    static SUSPENDED: std::cell::Cell<Option<Option<crate::clock::Instant>>> =
        const { std::cell::Cell::new(None) };
    /// The generated-code function the loop calls closures through, kept for
    /// `dartforge_resume`.
    static LOOP_CALL: std::cell::Cell<Option<extern "C" fn(i64) -> i64>> =
        const { std::cell::Cell::new(None) };
}

/// Whether the last event loop returned suspended (WebAssembly).
#[cfg(target_arch = "wasm32")]
fn event_loop_suspended() -> bool {
    SUSPENDED.with(|s| s.get().is_some())
}

/// Microseconds until the next timer of a suspended loop (0 when already due;
/// `f64::INFINITY` for a timer too far away to represent).
#[cfg(target_arch = "wasm32")]
fn event_loop_delay_micros() -> f64 {
    match SUSPENDED.with(|s| s.get()) {
        Some(Some(due)) => due.saturating_duration_since(crate::clock::Instant::now()).as_micros() as f64,
        Some(None) => f64::INFINITY,
        None => 0.0,
    }
}

/// Runs a suspended loop again (`dartforge_resume`).
#[cfg(target_arch = "wasm32")]
fn resume_event_loop() {
    SUSPENDED.with(|s| s.set(None));
    if let Some(call) = LOOP_CALL.with(|c| c.get()) {
        dartforge_event_loop(call);
    }
}

/// Where the native loop waits for `due` (or for a message), a module returns
/// to the host instead.
#[cfg(target_arch = "wasm32")]
fn suspend_event_loop(call: extern "C" fn(i64) -> i64, due: Option<crate::clock::Instant>) {
    LOOP_CALL.with(|c| c.set(Some(call)));
    SUSPENDED.with(|s| s.set(Some(due)));
}

// Current port/message roots are single-frame state. Until nested turns have
// their own roots and scheduling contract, reject entry before touching queues
// or serving work inside an active Dart/native callback frame.
struct EventLoopGuard;

impl EventLoopGuard {
    fn enter() -> Option<Self> {
        if in_owner_ffi_callback() || EVENT_LOOP_ACTIVE.with(|active| active.get()) {
            return None;
        }
        EVENT_LOOP_ACTIVE.with(|active| active.set(true));
        Some(Self)
    }
}

impl Drop for EventLoopGuard {
    fn drop(&mut self) {
        EVENT_LOOP_ACTIVE.with(|active| active.set(false));
    }
}

/// The SDK owns callbacks still linked inside its active microtask drain.
struct MicrotaskDrainGuard(bool);

impl MicrotaskDrainGuard {
    fn enter() -> Self {
        Self(MICROTASK_DRAIN_ACTIVE.with(|active| active.replace(true)))
    }
}

impl Drop for MicrotaskDrainGuard {
    fn drop(&mut self) {
        MICROTASK_DRAIN_ACTIVE.with(|active| active.set(self.0));
    }
}

impl Events {
    /// Discard canceled frontier entries before selecting an active timer.
    fn next_active_timer(&mut self) -> Option<(i64, i64, i64)> {
        loop {
            let (&(deadline, sequence), &id) = self.queue.first_key_value()?;
            if self.active.contains_key(&id) {
                return Some((deadline, sequence, id));
            }
            self.queue.remove(&(deadline, sequence));
        }
    }

    fn timer_instant(&self, deadline: i64) -> Option<crate::clock::Instant> {
        self.start?
            .checked_add(std::time::Duration::from_millis(deadline.max(0) as u64))
    }

    fn timer_delay_micros(&mut self, now: crate::clock::Instant) -> i64 {
        let Some((deadline, _, _)) = self.next_active_timer() else {
            return -1;
        };
        let Some(due) = self.timer_instant(deadline) else {
            return i64::MAX;
        };
        if due <= now {
            return 0;
        }
        // Preserve a positive delay even during the final fractional microsecond.
        i64::try_from(due.duration_since(now).as_micros())
            .unwrap_or(i64::MAX)
            .max(1)
    }

    /// Milliseconds of the isolate's monotonic clock.
    fn current_time(&mut self) -> i64 {
        let start = *self.start.get_or_insert_with(crate::clock::Instant::now);
        i64::try_from(start.elapsed().as_millis()).unwrap_or(i64::MAX)
    }

    /// A new root for `handle` (negative id, see the header).
    fn root_it(&mut self, handle: i64) -> i64 {
        self.next_root += 1;
        let id = -self.next_root;
        HEAP.with(|h| h.borrow_mut().set_global_root(id, handle));
        id
    }
}

/// Whether the owner has runnable work beyond its timer frontier.
fn event_ready_without_timers() -> bool {
    if MICROTASK_DRAIN_ACTIVE.with(|active| active.get()) {
        return true;
    }
    if EVENTS.with(|events| !events.borrow().immediates.is_empty()) {
        return true;
    }
    if HEAP.with(|heap| !heap.borrow().ready_finalizations.is_empty()) {
        return true;
    }
    if QUEUE.with(|queue| {
        let messages = queue.messages.lock().unwrap_or_else(|e| e.into_inner());
        !messages.normal.is_empty()
            || !messages.control.is_empty()
            || !messages.safepoints.is_empty()
    }) {
        return true;
    }
    NATIVE_PUMP_ENABLED.with(|enabled| enabled.get())
        && NATIVE_PUMP_DISPATCH_READY.with(|ready| ready.get())
        && EVENTS.with(|events| events.borrow().native_dispatcher.is_some())
}

/// Snapshot of the current owner's next runnable deadline, in microseconds.
/// `-1` means no runnable deadline (including a paused owner), `0` means ready
/// work, and a positive value is relative to the current monotonic clock.
/// This snapshot neither pumps events nor promises progress on another isolate.
///
/// ```
/// assert!(dartforge_runtime::abi::dartforge_native_DartForge_event_next_deadline_micros() >= -1);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_event_next_deadline_micros() -> i64 {
    if paused_isolate() {
        return -1;
    }
    if event_ready_without_timers() {
        return 0;
    }
    EVENTS.with(|events| {
        events
            .borrow_mut()
            .timer_delay_micros(crate::clock::Instant::now())
    })
}

/// Recheck readiness at the blocking boundary, without holding queue/heap borrows.
fn collect_at_event_idle_boundary() -> bool {
    if paused_isolate()
        || dartforge_exception_pending() != 0
        || dartforge_native_DartForge_event_next_deadline_micros() == 0
    {
        return false;
    }
    gc_policy_collect_if_idle()
}

fn event_wait_deadline(timer: Option<crate::clock::Instant>) -> Option<crate::clock::Instant> {
    match (timer, gc_policy_idle_deadline()) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn release_root(id: i64) {
    HEAP.with(|h| h.borrow_mut().set_global_root(id, 0));
}

/// A one-shot event moves its owner out of the queue; a periodic one borrows from
/// the active copy, which stays alive even if the callback cancels the timer's
/// persistent owner.
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

fn replace_native_event_dispatcher(closure: Option<i64>) {
    let previous = EVENTS.with(|events| {
        let mut events = events.borrow_mut();
        let replacement = closure.map(|value| (events.root_it(value), value));
        std::mem::replace(&mut events.native_dispatcher, replacement)
    });
    if let Some((root, _)) = previous {
        release_root(root);
    }
}

fn dispatch_native_events(call: extern "C" fn(i64) -> i64) -> bool {
    if paused_isolate() || !NATIVE_PUMP_ENABLED.with(|enabled| enabled.get()) {
        return false;
    }
    if !NATIVE_PUMP_DISPATCH_READY.with(|ready| ready.replace(false)) {
        return false;
    }
    let closure = EVENTS.with(|events| events.borrow().native_dispatcher.map(|(_, value)| value));
    let Some(closure) = closure else {
        return false;
    };
    // The active owning frame is independent of the persistent registration:
    // clearing/replacing the callback, or disabling the pump, cannot free it.
    in_native_scope_if_arc(|| call_event(call, closure, None));
    true
}

/// `_AsyncRun._scheduleImmediate(callback)`: the closure runs before the next
/// timer, after the ones already waiting.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_scheduleImmediate(closure: i64) {
    EVENTS.with(|e| {
        let mut e = e.borrow_mut();
        let root = e.root_it(closure);
        e.immediates.push_back((root, closure));
    });
    native_pump_signal_current();
}

/// `_Timer._create(ms, fire, periodic)`: schedules the timer and returns the id
/// (never 0). `fire` is the closure the loop calls at the deadline.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_Timer_new(ms: i64, fire: i64, periodic: u8) -> i64 {
    let id = EVENTS.with(|e| {
        let mut e = e.borrow_mut();
        let ms = ms.max(0);
        let current_time = e.current_time();
        let deadline = if ms == 0 {
            current_time
        } else {
            current_time.saturating_add(1).saturating_add(ms)
        };
        e.next_id += 1;
        let id = e.next_id;
        let root = e.root_it(fire);
        e.next_seq += 1;
        let seq = e.next_seq;
        e.queue.insert((deadline, seq), id);
        e.active.insert(
            id,
            LoopTimer {
                closure: fire,
                root,
                ms,
                periodic: periodic != 0,
                tick: 0,
            },
        );
        id
    });
    native_pump_signal_current();
    id
}

/// The callback count includes periods skipped while the isolate was blocked,
/// as in the SDK's `_Timer._runTimers`. An inactive timer has no runtime tick.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_Timer_tick(id: i64) -> i64 {
    EVENTS.with(|e| e.borrow().active.get(&id).map_or(0, |t| t.tick))
}

/// Advance a positive-period deadline to the last missed period before firing.
/// The strict comparison intentionally preserves the SDK's overdue == period
/// boundary: that case still permits another immediate callback.
fn periodic_firing(deadline: i64, current_time: i64, ms: i64) -> (i64, i64) {
    if ms <= 0 {
        return (deadline, 1);
    }
    let overdue = current_time.saturating_sub(deadline);
    let missed = if overdue > ms { overdue / ms } else { 0 };
    (
        deadline.saturating_add(missed.saturating_mul(ms)),
        missed.saturating_add(1),
    )
}

/// `_Timer._cancelNative(id)`: removes the timer from the queue and releases the root.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_Timer_cancel(id: i64) {
    let root = EVENTS.with(|e| e.borrow_mut().active.remove(&id).map(|t| t.root));
    if let Some(r) = root {
        release_root(r);
        native_pump_signal_current();
    }
}

/// The event loop: runs the pending microtasks and the timers until there is
/// nothing left (or an exception leaves one of them). `call` is the
/// generated-code function that calls a closure without arguments.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_event_loop(call: extern "C" fn(i64) -> i64) {
    let Some(_loop_guard) = EventLoopGuard::enter() else {
        if dartforge_exception_pending() == 0 {
            let error = allocate_state_error("Nested Dart event turns are unavailable");
            with_roots(&[error], || dartforge_exception_throw(error, 3));
        }
        return;
    };
    // In the loop, the isolate serves requests at the safepoint (`ports.rs`).
    register_live_isolate();
    let mut native_poll_budget = 0u8;
    loop {
        if dartforge_exception_pending() != 0 {
            return;
        }
        // Bound ready Dart work between nonblocking native pump passages.
        // This cannot preempt a long callback or the SDK's own microtask drain.
        native_poll_budget += 1;
        if native_poll_budget >= 8 {
            native_poll_budget = 0;
            if !paused_isolate() {
                native_pump_poll();
                if dartforge_exception_pending() != 0 {
                    return;
                }
            }
        }
        // 1. Microtasks, all of them, before any timer.
        let immediate_one = EVENTS.with(|e| e.borrow_mut().immediates.pop_front());
        if let Some((root, closure)) = immediate_one {
            // Each event in its own native scope: the owners that the Dart ports
            // return die at the end of the event, not of the loop.
            let _drain = MicrotaskDrainGuard::enter();
            in_native_scope_if_arc(|| call_event(call, closure, Some(root)));
            continue;
        }
        // 1b. The ready finalizations (`Finalizer`, `finalizers.rs`): in the
        //     VM they arrive as a message to the isolate; here, between events.
        if let Some((frame, action)) = start_finalization() {
            in_native_scope_if_arc(|| dart_r1(call as usize, action));
            finish_finalization(frame);
            continue;
        }
        // 2. The requests at the safepoint (the publication of a JIT reload,
        //    `ports.rs`) and the control messages (`isolates.rs`), between one
        //    event and the next; when paused, the isolate only serves those two.
        serve_safepoints();
        if !serve_control() {
            return;
        }
        if paused_isolate() {
            wait_message(None, true);
            continue;
        }
        // Native event work runs only after CF has returned, after microtasks
        // and controls. Never dispatch it inside a CF/foreign callback frame.
        if dispatch_native_events(call) {
            continue;
        }
        // 3. The next event: the timer with the smallest (deadline, id) or the
        //    oldest message in the isolate's queue (`ports.rs`), by arrival
        //    order — in the VM the timer also arrives as a message. With neither,
        //    the isolate waits while it has a live port.
        let next_offset = EVENTS.with(|e| e.borrow_mut().next_active_timer());
        let due = next_offset
            .and_then(|(deadline, _, _)| EVENTS.with(|e| e.borrow().timer_instant(deadline)));
        if let Some(arrival) = next_arrival()
            && due.is_none_or(|v| arrival < v)
        {
            in_native_scope_if_arc(|| dispatch_next(call));
            continue;
        }
        let Some((deadline, seq, id)) = next_offset else {
            // A module's only isolate has nobody else to send it a message: a
            // live port alone ends the program, as dart2wasm's does when the
            // JavaScript event loop empties.
            if has_live_port() && !cfg!(target_arch = "wasm32") {
                if collect_at_event_idle_boundary() {
                    continue;
                }
                wait_message(event_wait_deadline(None), false);
                continue;
            }
            return;
        };
        let Some(due) = due else {
            // A representably huge timer stays pending without overflowing Instant.
            if collect_at_event_idle_boundary() {
                continue;
            }
            #[cfg(target_arch = "wasm32")]
            {
                suspend_event_loop(call, None);
                return;
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                wait_message(event_wait_deadline(None), false);
                continue;
            }
        };
        if crate::clock::Instant::now() < due {
            // Waits for the deadline, waking earlier if a message arrives.
            if collect_at_event_idle_boundary() {
                continue;
            }
            #[cfg(target_arch = "wasm32")]
            {
                suspend_event_loop(call, Some(due));
                return;
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                wait_message(event_wait_deadline(Some(due)), false);
                continue;
            }
        }
        EVENTS.with(|e| e.borrow_mut().queue.remove(&(deadline, seq)));
        let firing = EVENTS.with(|e| {
            let mut e = e.borrow_mut();
            let (closure, periodic, ms) = {
                let t = e.active.get(&id)?;
                (t.closure, t.periodic, t.ms)
            };
            if periodic {
                // The clock only matters to the catch-up of a periodic timer:
                // a one-shot one fires without reading it.
                let current_time = e.current_time();
                let (adjusted_deadline, delta) = periodic_firing(deadline, current_time, ms);
                let t = e.active.get_mut(&id).expect("active timer");
                t.tick = t.tick.saturating_add(delta);
                return Some((closure, true, None, adjusted_deadline));
            }
            let t = e.active.remove(&id).expect("active timer");
            Some((closure, false, Some(t.root), deadline))
        });
        let Some((closure, periodic, root, adjusted_deadline)) = firing else {
            continue;
        };
        in_native_scope_if_arc(|| call_event(call, closure, root));
        if periodic {
            // Like the VM (`_runTimers`): rescheduled AFTER the callback, with a
            // new sequence, if the callback did not cancel it.
            EVENTS.with(|e| {
                let mut e = e.borrow_mut();
                let Some(ms) = e.active.get(&id).map(|t| t.ms) else {
                    return;
                };
                let new = if ms > 0 {
                    adjusted_deadline.saturating_add(ms)
                } else {
                    e.current_time()
                };
                e.next_seq += 1;
                let seq = e.next_seq;
                e.queue.insert((new, seq), id);
            });
        }
    }
}

/// `_trySetStackTrace(error, stackTrace)` (the VM's `Error_trySetStackTrace`):
/// writes the stack trace into an `Error` that does not have one yet. Program
/// and source errors keep the stack trace in their own layout (the source's
/// `Error`, P5); the runtime's (ids 1000–1012) already receive it at `throw`.
/// Until `Error` comes from source, there is nothing to write.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Error_trySetStackTrace(_error: i64, _trace: i64) {}

#[cfg(test)]
mod tests_event_loop_reentry {
    use super::*;

    thread_local! {
        static EXECUTED: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) };
    }

    extern "C" fn never_run(_: i64) -> i64 {
        panic!("refused nested loop dispatched queued work");
    }

    extern "C" fn reenter_then_catch(closure: i64) -> i64 {
        EXECUTED.with(|executed| executed.borrow_mut().push(closure));
        let before = EXECUTED.with(|executed| executed.borrow().len());
        dartforge_event_loop(never_run);
        assert_eq!(dartforge_exception_pending(), 1);
        assert_eq!(EXECUTED.with(|executed| executed.borrow().len()), before);
        assert!(EVENT_LOOP_ACTIVE.with(|active| active.get()));
        dartforge_exception_clear();
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            heap.collect();
            assert!(heap.is_live_object(closure), "active callback lost its root");
        });
        0
    }

    #[test]
    fn rejected_entry_preserves_queues_current_message_and_existing_exception() {
        let previous_heap = HEAP.with(|heap| heap.replace(Heap::new(true)));
        let previous_events = EVENTS.with(|events| events.replace(Events::default()));
        let previous_current = CURRENT.with(|current| current.replace((17, 0)));
        let closure = HEAP.with(|heap| heap.borrow_mut().new_closure(123, (0, false), 0, 0));
        HEAP.with(|heap| heap.borrow_mut().set_global_root(MESSAGE_ROOT, closure));
        CURRENT.with(|current| { current.replace((17, closure)); });
        dartforge_native_DartForge_scheduleImmediate(closure);
        let timer = dartforge_native_DartForge_Timer_new(100, closure, 0);
        let was_registered = LIVE_REGISTERED.with(|live| live.get());
        let guard = EventLoopGuard::enter().unwrap();
        dartforge_event_loop(never_run);
        assert_eq!(dartforge_exception_pending(), 1);
        let error = dartforge_exception_peek_ref();
        HEAP.with(|heap| assert_eq!(heap.borrow().object(error).unwrap().class_id, 1002));
        dartforge_event_loop(never_run);
        assert_eq!(dartforge_exception_peek_ref(), error, "existing exception was overwritten");
        assert_eq!(CURRENT.with(|current| *current.borrow()), (17, closure));
        assert_eq!(LIVE_REGISTERED.with(|live| live.get()), was_registered);
        EVENTS.with(|events| {
            let events = events.borrow();
            assert_eq!(events.immediates.len(), 1);
            assert!(events.active.contains_key(&timer));
        });
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            heap.collect();
            assert!(heap.is_live_object(closure));
        });
        dartforge_exception_clear();
        let original_error = allocate_state_error("Original outer exception");
        with_roots(&[original_error], || dartforge_exception_throw(original_error, 3));
        dartforge_event_loop(never_run);
        assert_eq!(dartforge_exception_peek_ref(), original_error);
        dartforge_exception_clear();
        // Leave only the message root. Its lifetime must not have been erased
        // by a refused nested dispatch, even after the queued roots are gone.
        dartforge_native_DartForge_Timer_cancel(timer);
        let queued_root = EVENTS.with(|events| events.borrow_mut().immediates.pop_front().unwrap().0);
        release_root(queued_root);
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            heap.collect();
            assert!(heap.is_live_object(closure), "message root was overwritten");
            heap.set_global_root(MESSAGE_ROOT, 0);
            heap.collect();
            assert!(!heap.is_live_object(closure));
        });
        drop(guard);
        assert!(EventLoopGuard::enter().is_some(), "refusal corrupted the outer guard");
        CURRENT.with(|current| { current.replace(previous_current); });
        EVENTS.with(|events| { events.replace(previous_events); });
        HEAP.with(|heap| { heap.replace(previous_heap); });
    }

    #[test]
    fn outer_turns_continue_after_caught_refusal_in_tracing_and_arc() {
        for arc in [false, true] {
            let previous_heap = HEAP.with(|heap| heap.replace(Heap::new(true)));
            let previous_events = EVENTS.with(|events| events.replace(Events::default()));
            EXECUTED.with(|executed| executed.borrow_mut().clear());
            if arc { HEAP.with(|heap| heap.borrow_mut().enable_arc()); }
            let mut closures = Vec::new();
            for code in [123, 456] {
                let closure = HEAP.with(|heap| heap.borrow_mut().new_closure(code, (0, false), 0, 0));
                dartforge_native_DartForge_scheduleImmediate(closure);
                closures.push(closure);
            }
            dartforge_event_loop(reenter_then_catch);
            EXECUTED.with(|executed| assert_eq!(executed.borrow().as_slice(), closures));
            assert_eq!(dartforge_exception_pending(), 0);
            assert!(!EVENT_LOOP_ACTIVE.with(|active| active.get()));
            EVENTS.with(|events| assert!(events.borrow().immediates.is_empty()));
            EVENTS.with(|events| { events.replace(previous_events); });
            HEAP.with(|heap| { heap.replace(previous_heap); });
        }
    }
}

#[cfg(test)]
mod tests_event_owners {
    use super::*;
    thread_local! { static TIMER: std::cell::Cell<i64> = const { std::cell::Cell::new(0) }; }

    extern "C" fn callback(closure: i64) -> i64 {
        let timer = TIMER.with(|t| t.get());
        if timer != 0 {
            dartforge_native_DartForge_Timer_cancel(timer);
        }
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(
                h.is_live_object(closure),
                "callback lost its owner during the call"
            );
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
            let id = dartforge_native_DartForge_Timer_new(0, closure, 1);
            TIMER.with(|t| t.set(id));
            call_event(callback, closure, None);
        } else {
            TIMER.with(|t| t.set(0));
            dartforge_native_DartForge_scheduleImmediate(closure);
            let (root, value) = EVENTS.with(|e| e.borrow_mut().immediates.pop_front().unwrap());
            call_event(callback, value, Some(root));
        }
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(
                !h.is_live_object(closure),
                "residual active owner after the callback"
            );
        });
        TIMER.with(|t| t.set(0));
        EVENTS.with(|e| {
            e.replace(previous_events);
        });
        HEAP.with(|h| {
            h.replace(previous_heap);
        });
    }

    #[test]
    fn single_event_transfers_queue_owner_and_releases_after_callback() {
        verify(false);
    }

    #[test]
    fn periodic_timer_cancelled_in_callback_keeps_active_owner() {
        verify(true);
    }

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
        dartforge_native_DartForge_scheduleImmediate(closure);
        let (root, value) = EVENTS.with(|e| e.borrow_mut().immediates.pop_front().unwrap());
        call_event(callback_throws, value, Some(root));
        assert_eq!(dartforge_exception_pending(), 1);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(
                h.is_live_object(closure),
                "the pending exception must keep its owner"
            );
        });
        dartforge_exception_clear();
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(
                !h.is_live_object(closure),
                "active or queue owner left residual"
            );
        });
        EVENTS.with(|e| {
            e.replace(previous_events);
        });
        HEAP.with(|h| {
            h.replace(previous_heap);
        });
    }
}

#[cfg(test)]
mod tests_periodic_timer_deadlines {
    use super::*;

    #[test]
    fn missed_periods_match_the_sdk_strict_boundary() {
        assert_eq!(periodic_firing(100, 100, 10), (100, 1));
        assert_eq!(periodic_firing(100, 109, 10), (100, 1));
        assert_eq!(periodic_firing(100, 110, 10), (100, 1));
        assert_eq!(periodic_firing(100, 111, 10), (110, 2));
        assert_eq!(periodic_firing(100, 130, 10), (130, 4));
        assert_eq!(periodic_firing(100, 135, 10), (130, 4));
    }

    #[test]
    fn callback_delay_is_accounted_at_the_next_firing() {
        let (first, delta) = periodic_firing(100, 100, 10);
        assert_eq!(delta, 1);
        let next = first + 10; // Rescheduled after the callback, preserving its phase.
        assert_eq!(periodic_firing(next, 145, 10), (140, 4));
        assert_eq!(periodic_firing(0, 0, 0), (0, 1));
        assert_eq!(periodic_firing(100, 10000, 0), (100, 1));
    }

    #[test]
    fn deadline_arithmetic_does_not_overflow() {
        assert_eq!(periodic_firing(i64::MAX - 30, i64::MAX, 10), (i64::MAX, 4));
        assert_eq!(periodic_firing(0, i64::MAX, 1), (i64::MAX, i64::MAX));
    }

    #[test]
    fn tick_lookup_observes_firing_and_cancellation() {
        let previous = EVENTS.with(|e| e.replace(Events::default()));
        EVENTS.with(|e| {
            e.borrow_mut().active.insert(
                42,
                LoopTimer {
                    closure: 0,
                    root: 0,
                    ms: 10,
                    periodic: true,
                    tick: 17,
                },
            );
        });
        assert_eq!(dartforge_native_DartForge_Timer_tick(42), 17);
        EVENTS.with(|e| {
            e.borrow_mut().active.remove(&42);
        });
        assert_eq!(dartforge_native_DartForge_Timer_tick(42), 0);
        EVENTS.with(|e| {
            e.replace(previous);
        });
    }
}

#[cfg(test)]
mod tests_native_dispatcher_roots {
    use super::*;
    thread_local! {
        static ACTION: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
        static REPLACEMENT: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
    }

    extern "C" fn callback(closure: i64) -> i64 {
        match ACTION.with(|action| action.get()) {
            0 => replace_native_event_dispatcher(None),
            1 => {
                let replacement =
                    HEAP.with(|heap| heap.borrow_mut().new_closure(456, (0, false), 0, 0));
                replace_native_event_dispatcher(Some(replacement));
                REPLACEMENT.with(|value| value.set(replacement));
            }
            2 => QUEUE.with(|queue| native_pump_detach(queue)),
            3 => dartforge_exception_throw(closure, 3),
            _ => unreachable!(),
        }
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            heap.collect();
            assert!(
                heap.is_live_object(closure),
                "active dispatcher lost its root"
            );
        });
        0
    }

    fn verify(action: u8, arc: bool) {
        let previous_heap = HEAP.with(|heap| heap.replace(Heap::new(true)));
        let previous_events = EVENTS.with(|events| events.replace(Events::default()));
        let previous_enabled = NATIVE_PUMP_ENABLED.with(|enabled| enabled.replace(true));
        let previous_ready = NATIVE_PUMP_DISPATCH_READY.with(|ready| ready.replace(true));
        let closure = HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            if arc {
                heap.enable_arc();
            }
            heap.new_closure(123, (0, false), 0, 0)
        });
        replace_native_event_dispatcher(Some(closure));
        ACTION.with(|value| value.set(action));
        assert!(dispatch_native_events(callback));
        if action == 3 {
            assert_eq!(dartforge_exception_pending(), 1);
            replace_native_event_dispatcher(None);
            HEAP.with(|heap| {
                let mut heap = heap.borrow_mut();
                heap.collect();
                assert!(
                    heap.is_live_object(closure),
                    "pending exception lost its owner"
                );
            });
            dartforge_exception_clear();
        }
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            heap.collect();
            assert!(!heap.is_live_object(closure));
            if action == 1 {
                let replacement = REPLACEMENT.with(|value| value.get());
                assert!(heap.is_live_object(replacement));
            }
        });
        replace_native_event_dispatcher(None);
        assert!(
            !dispatch_native_events(callback),
            "dispatcher ran without new native work"
        );
        NATIVE_PUMP_ENABLED.with(|enabled| enabled.set(previous_enabled));
        NATIVE_PUMP_DISPATCH_READY.with(|ready| ready.set(previous_ready));
        EVENTS.with(|events| {
            events.replace(previous_events);
        });
        HEAP.with(|heap| {
            heap.replace(previous_heap);
        });
    }

    #[test]
    fn self_clear_preserves_active_root_in_tracing_and_arc() {
        verify(0, false);
        verify(0, true);
    }
    #[test]
    fn self_replace_preserves_both_lifetimes() {
        verify(1, false);
        verify(1, true);
    }
    #[test]
    fn disable_clears_registration_after_active_callback() {
        verify(2, false);
        verify(2, true);
    }
    #[test]
    fn exception_survives_registration_cleanup() {
        verify(3, false);
        verify(3, true);
    }
    #[test]
    fn foreign_owner_cannot_register_or_clear() {
        let (supported, set, clear) = std::thread::spawn(|| {
            (
                dartforge_native_DartForge_native_pump_dispatcher_supported(),
                dartforge_native_DartForge_native_pump_set_dispatcher(0),
                dartforge_native_DartForge_native_pump_clear_dispatcher(),
            )
        })
        .join()
        .unwrap();
        assert_eq!(supported, 0);
        let expected = if cfg!(target_os = "macos") { -1 } else { 0 };
        assert_eq!(set, expected);
        assert_eq!(clear, expected);
    }
}

#[cfg(test)]
mod tests_event_deadline_snapshot {
    use super::*;

    struct Snapshot {
        events: Events,
        heap: Heap,
        queues: Queues,
        isolate: Option<IsolateState>,
        enabled: bool,
        ready: bool,
    }
    impl Snapshot {
        fn new() -> Self {
            Self {
                events: EVENTS.with(|state| state.replace(Events::default())),
                heap: HEAP.with(|state| state.replace(Heap::new(true))),
                queues: QUEUE.with(|queue| std::mem::take(&mut *queue.messages.lock().unwrap())),
                isolate: ISOLATE.with(|state| state.replace(None)),
                enabled: NATIVE_PUMP_ENABLED.with(|state| state.replace(false)),
                ready: NATIVE_PUMP_DISPATCH_READY.with(|state| state.replace(false)),
            }
        }
    }
    impl Drop for Snapshot {
        fn drop(&mut self) {
            EVENTS.with(|state| {
                state.replace(std::mem::take(&mut self.events));
            });
            HEAP.with(|state| std::mem::swap(&mut *state.borrow_mut(), &mut self.heap));
            QUEUE.with(|queue| {
                std::mem::swap(&mut *queue.messages.lock().unwrap(), &mut self.queues)
            });
            ISOLATE.with(|state| {
                state.replace(self.isolate.take());
            });
            NATIVE_PUMP_ENABLED.with(|state| state.set(self.enabled));
            NATIVE_PUMP_DISPATCH_READY.with(|state| state.set(self.ready));
        }
    }
    fn active(events: &mut Events, id: i64, deadline: i64) {
        events.queue.insert((deadline, id), id);
        events.active.insert(
            id,
            LoopTimer {
                closure: 0,
                root: 0,
                ms: 0,
                periodic: false,
                tick: 0,
            },
        );
    }

    #[test]
    fn active_deadlines_use_submillisecond_relative_clock_and_clean_cancellations() {
        let start = crate::clock::Instant::now();
        let mut events = Events {
            start: Some(start),
            ..Events::default()
        };
        events.queue.insert((1, 90), 90); // Stale canceled timer before the live frontier.
        active(&mut events, 1, 20);
        assert_eq!(
            events.timer_delay_micros(start + std::time::Duration::from_micros(10_750)),
            9_250
        );
        assert!(!events.queue.contains_key(&(1, 90)));
        assert_eq!(
            events.timer_delay_micros(start + std::time::Duration::from_millis(20)),
            0
        );
        events.active.clear();
        assert_eq!(events.timer_delay_micros(start), -1);
        assert!(events.queue.is_empty());
    }

    #[test]
    fn fractional_microsecond_future_is_positive_and_large_deadlines_saturate() {
        let start = crate::clock::Instant::now();
        let mut events = Events {
            start: Some(start),
            ..Events::default()
        };
        active(&mut events, 1, 1);
        assert_eq!(
            events.timer_delay_micros(start + std::time::Duration::from_nanos(999_999)),
            1
        );
        events.queue.clear();
        active(&mut events, 2, i64::MAX);
        assert_eq!(events.timer_delay_micros(start), i64::MAX);
    }

    #[test]
    fn queued_work_overrides_future_timers_without_consuming_it() {
        let _snapshot = Snapshot::new();
        assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), -1);
        EVENTS.with(|events| {
            let mut events = events.borrow_mut();
            events.start = Some(crate::clock::Instant::now());
            active(&mut events, 1, 10_000);
            events.immediates.push_back((0, 0));
        });
        assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), 0);
        EVENTS.with(|events| {
            assert_eq!(events.borrow_mut().immediates.pop_front(), Some((0, 0)));
        });
        QUEUE.with(|queue| {
            queue.messages.lock().unwrap().normal.push_back(Message {
                port: 0,
                graph: Graph::null_val(),
                owners: vec![],
                arrival: crate::clock::Instant::now(),
            })
        });
        assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), 0);
        QUEUE.with(|queue| {
            queue.messages.lock().unwrap().normal.clear();
        });
        QUEUE.with(|queue| {
            queue
                .messages
                .lock()
                .unwrap()
                .control
                .push_back(Graph::null_val())
        });
        assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), 0);
        QUEUE.with(|queue| {
            queue.messages.lock().unwrap().control.clear();
        });
        let closure = HEAP.with(|heap| heap.borrow_mut().new_closure(123, (0, false), 0, 0));
        HEAP.with(|heap| heap.borrow_mut().add_ready_finalization(closure));
        assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), 0);
        assert!(!HEAP.with(|heap| heap.borrow().ready_finalizations.is_empty()));
    }

    #[test]
    fn only_dispatchable_native_work_is_ready_and_paused_owner_promises_nothing() {
        let _snapshot = Snapshot::new();
        NATIVE_PUMP_DISPATCH_READY.with(|state| state.set(true));
        assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), -1);
        NATIVE_PUMP_ENABLED.with(|state| state.set(true));
        assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), -1);
        EVENTS.with(|events| {
            events.borrow_mut().native_dispatcher = Some((0, 0));
        });
        assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), 0);
        assert!(NATIVE_PUMP_DISPATCH_READY.with(|state| state.get()));
        ISOLATE.with(|state| {
            state.replace(Some(IsolateState {
                control: 0,
                pause: 0,
                termination: 0,
                errors_fatal: true,
                pauses: vec![1],
                exit_listeners: vec![],
                error_listeners: vec![],
                shut_down: false,
                shut_down_now: false,
            }));
        });
        assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), -1);
        HEAP.with(|heap| heap.borrow_mut().gc_policy_request_idle());
        let collections = HEAP.with(|heap| heap.borrow().stats().collections);
        assert!(!collect_at_event_idle_boundary());
        assert_eq!(
            HEAP.with(|heap| heap.borrow().stats().collections),
            collections
        );
    }
    thread_local! {
        static EXECUTED: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) };
    }

    extern "C" fn observe_microtask(closure: i64) -> i64 {
        assert!(EVENTS.with(|events| events.borrow().immediates.is_empty()));
        assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), 0);
        EXECUTED.with(|executed| executed.borrow_mut().push(closure));
        0
    }

    extern "C" fn observe_order(closure: i64) -> i64 {
        EXECUTED.with(|executed| executed.borrow_mut().push(closure));
        0
    }

    #[test]
    fn actual_sdk_microtask_drain_reports_ready_and_restores_on_return_and_unwind() {
        let _snapshot = Snapshot::new();
        EXECUTED.with(|executed| executed.borrow_mut().clear());
        let registered = LIVE_REGISTERED.with(|registered| registered.get());
        let closure = HEAP.with(|heap| heap.borrow_mut().new_closure(123, (0, false), 0, 0));
        dartforge_native_DartForge_scheduleImmediate(closure);
        dartforge_event_loop(observe_microtask);
        if !registered {
            remove_live_isolate();
        }
        EXECUTED.with(|executed| assert_eq!(&*executed.borrow(), &[closure]));
        assert!(!MICROTASK_DRAIN_ACTIVE.with(|active| active.get()));
        assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), -1);
        assert!(
            std::panic::catch_unwind(|| {
                let _guard = MicrotaskDrainGuard::enter();
                assert_eq!(dartforge_native_DartForge_event_next_deadline_micros(), 0);
                panic!("Expected microtask guard unwind");
            })
            .is_err()
        );
        assert!(!MICROTASK_DRAIN_ACTIVE.with(|active| active.get()));
    }

    #[test]
    fn idle_collection_expiry_wakes_before_timer_and_runs_generated_finalizer_first() {
        let _snapshot = Snapshot::new();
        HEAP.with(|heap| {
            heap.replace(Heap::new(false));
        });
        EXECUTED.with(|executed| executed.borrow_mut().clear());
        let registered = LIVE_REGISTERED.with(|registered| registered.get());
        let (action, timer) = HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            let owner = heap.new_object(1234, &[]);
            heap.set_global_root(42, owner);
            let value = heap.new_object(1235, &[]);
            let action = heap.new_closure(123, (0, false), 0, 0);
            heap.add_attachment(crate::heap::FinalizerAttachment {
                owner,
                value,
                detach: 0,
                action: crate::heap::FinalizerAction::Dart(action),
            });
            let timer = heap.new_closure(456, (0, false), 0, 0);
            (action, timer)
        });
        dartforge_native_DartForge_Timer_new(100, timer, 0);
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            assert_eq!(
                heap.gc_policy_defer(std::time::Duration::from_millis(20), 1024),
                1
            );
            heap.gc_policy_request_idle();
        });
        let policy = gc_policy_idle_deadline().unwrap();
        let timer_due = EVENTS.with(|events| {
            let mut events = events.borrow_mut();
            let (deadline, _, _) = events.next_active_timer().unwrap();
            events.timer_instant(deadline).unwrap()
        });
        assert_eq!(event_wait_deadline(Some(timer_due)), Some(policy));
        dartforge_event_loop(observe_order);
        if !registered {
            remove_live_isolate();
        }
        EXECUTED.with(|executed| assert_eq!(&*executed.borrow(), &[action, timer]));
        assert!(HEAP.with(|heap| heap.borrow().ready_finalizations.is_empty()));
        assert!(gc_policy_idle_deadline().is_none());
    }
}
