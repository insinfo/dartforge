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
// `Error.throwWithStackTrace`): the loop stops and `finalizar_programa` reports
// it, as the VM does with "Unhandled exception".
//
// The stored closures are collector roots while they wait: each one takes a heap
// root global (`set_global_root`) with a NEGATIVE id — the program's globals use
// the non-negative ids.
//
// Per-thread state (one isolate per thread; P8 gathers everything in `isolado.rs`).

struct LoopTimer {
    /// The closure that fires the timer (`_Timer._disparar` with the receiver).
    closure: i64,
    root: i64,
    ms: i64,
    periodic: bool,
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
    start: Option<std::time::Instant>,
}

thread_local! {
    static EVENTS: RefCell<Events> = RefCell::new(Events::default());
}

impl Events {
    /// Milliseconds of the isolate's monotonic clock.
    fn current_time(&mut self) -> i64 {
        let start = *self.start.get_or_insert_with(std::time::Instant::now);
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

/// `_AsyncRun._scheduleImmediate(callback)`: the closure runs before the next
/// timer, after the ones already waiting.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_scheduleImmediate(closure: i64) {
    EVENTS.with(|e| {
        let mut e = e.borrow_mut();
        let root = e.root_it(closure);
        e.immediates.push_back((root, closure));
    });
}

/// `_Timer._novo(ms, fire, periodic)`: schedules the timer and returns the id
/// (never 0). `fire` is the closure the loop calls at the deadline.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_Timer_new(ms: i64, fire: i64, periodic: u8) -> i64 {
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

/// `_Timer._cancelar(id)`: removes the timer from the queue and releases the root.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_Timer_cancel(id: i64) {
    let root = EVENTS.with(|e| e.borrow_mut().active.remove(&id).map(|t| t.root));
    if let Some(r) = root {
        release_root(r);
    }
}

/// The event loop: runs the pending microtasks and the timers until there is
/// nothing left (or an exception leaves one of them). `call` is the
/// generated-code function that calls a closure without arguments.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_event_loop(call: extern "C" fn(i64) -> i64) {
    // In the loop, the isolate serves requests at the safepoint (`ports.rs`).
    register_live_isolate();
    loop {
        if dartforge_exception_pending() != 0 {
            return;
        }
        // 1. Microtasks, all of them, before any timer.
        let immediate_one = EVENTS.with(|e| e.borrow_mut().immediates.pop_front());
        if let Some((root, closure)) = immediate_one {
            // Each event in its own native scope: the owners that the Dart ports
            // return die at the end of the event, not of the loop.
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
        // 3. The next event: the timer with the smallest (deadline, id) or the
        //    oldest message in the isolate's queue (`ports.rs`), by arrival
        //    order — in the VM the timer also arrives as a message. With neither,
        //    the isolate waits while it has a live port.
        let next_offset = EVENTS.with(|e| {
            let mut e = e.borrow_mut();
            loop {
                let (&(deadline, seq), &id) = e.queue.iter().next()?;
                if e.active.contains_key(&id) {
                    return Some((deadline, seq, id));
                }
                // Cancelled: the queue entry is discarded.
                e.queue.remove(&(deadline, seq));
            }
        });
        let due = next_offset.map(|(deadline, _, _)| {
            EVENTS.with(|e| {
                let mut e = e.borrow_mut();
                e.current_time();
                e.start.expect("clock started") + std::time::Duration::from_millis(deadline.max(0) as u64)
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
        let due = due.expect("timer with a deadline");
        if std::time::Instant::now() < due {
            // Waits for the deadline, waking earlier if a message arrives.
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
            // Like the VM (`_runTimers`): rescheduled AFTER the callback, with a
            // new sequence, if the callback did not cancel it.
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

/// `_trySetStackTrace(error, stackTrace)` (the VM's `Error_trySetStackTrace`):
/// writes the stack trace into an `Error` that does not have one yet. Program
/// and source errors keep the stack trace in their own layout (the source's
/// `Error`, P5); the runtime's (ids 1000–1012) already receive it at `throw`.
/// Until `Error` comes from source, there is nothing to write.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Error_trySetStackTrace(_error: i64, _trace: i64) {}

#[cfg(test)]
mod tests_event_owners {
    use super::*;
    thread_local! { static TIMER: std::cell::Cell<i64> = const { std::cell::Cell::new(0) }; }

    extern "C" fn callback(closure: i64) -> i64 {
        let timer = TIMER.with(|t| t.get());
        if timer != 0 { dartforge_native_DartForge_Timer_cancel(timer); }
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(h.is_live_object(closure), "callback lost its owner during the call");
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
            assert!(!h.is_live_object(closure), "residual active owner after the callback");
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
        dartforge_native_DartForge_scheduleImmediate(closure);
        let (root, value) = EVENTS.with(|e| e.borrow_mut().immediates.pop_front().unwrap());
        call_event(callback_throws, value, Some(root));
        assert_eq!(dartforge_exception_pending(), 1);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(h.is_live_object(closure), "the pending exception must keep its owner");
        });
        dartforge_exception_clear();
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.is_live_object(closure), "active or queue owner left residual");
        });
        EVENTS.with(|e| { e.replace(previous_events); });
        HEAP.with(|h| { h.replace(previous_heap); });
    }
}
