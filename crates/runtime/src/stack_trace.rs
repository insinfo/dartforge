// The stack trace in the VM's format (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
// §13.14): the address → (function, line, column) table that each image compiled
// with `--rastro=simbolico` registers at startup, the return addresses stored at
// `throw` and the symbolization, done only when the text is requested (`toString`,
// the uncaught exception).
//
// A table entry (`emit_native/src/llvm/rastro.rs`) has 12 bytes: the label and the
// function record, each as an offset from its own field, and a word. The two low
// bits of the record field (the record is aligned to 4) say the kind: 0, a call
// site (the word is `line << 12 | column`; entries with the same label are the
// inlined frames, from the inside out); 1, the identity of a function through the
// uniform entry of its closures (the word is the token); 2, the `await` of each
// state of an `async` body, in order; 3, the link of a closure with its awaiter
// (`position << 2 | direct << 1 | cell`). The function record is the offset to the
// url, one byte of marks and the name, the strings NUL-terminated.
//
// **The awaiter chain** (the VM's `AsyncAwareStackUnwinder`,
// `runtime/vm/stack_trace.cc`). The resumed `async` body and
// `_FutureListener.handleValue` push the frame or the listener onto the stack-trace
// stack (`dartforge_rastro_entrar`); at `throw`, the walk of the machine frames
// matches each marked frame with its entry and, once the first already suspended
// `async` body (or the listener) is found, follows the chain through the heap:
// frame → `Completer` → `_Future` → `_FutureListener` → the awaiting closure,
// through the links of the captures marked `@pragma('vm:awaiter-link')`, up to the
// frame of the next suspended body. Each one comes out as `<asynchronous
// suspension>` followed by its frame, at the position of the `await`. The stream
// branches too (`UnwindFrameToStreamController`): the `async*` body goes to its
// controller (`_AsyncStarStreamController.controller`), and a subscribed controller,
// to the subscription's `onData`; `await for` follows the `_Future` of `moveNext`
// through the `_StreamIterator` of the `_onData` tear-off, and `yield*`, the
// `asyncStarBody` of the destination controller through `addStreamFuture`.
//
// A runtime fragment: no `use` (the fragments are a single program).
// Written without compiling or running (2026-10-05).

/// The registered stack-trace sections, `(start, end)`.
static TRACE_SECTIONS: std::sync::Mutex<Vec<(usize, usize)>> = std::sync::Mutex::new(Vec::new());
/// Some image registered the table: `throw` stores the addresses.
static HAS_TRACE_TABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// What the sections say, and the generation of the sections it was built with:
/// built on the first request, rebuilt when a section comes in or goes out.
static TRACE_INDEX: std::sync::RwLock<(usize, TraceIndex)> =
    std::sync::RwLock::new((0, TraceIndex { points: Vec::new(), functions: Vec::new() }));
/// The generation of the sections: grows on every registration and every removal
/// (the JIT removes the section of the object it releases). Starts at 1: the empty
/// index is generation 0.
static TRACE_GENERATION: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
/// The `dart:async` fields that the awaiter chain reads:
/// `(class id, "Class.field", position)`.
static TRACE_FIELDS: std::sync::Mutex<Vec<(i64, String, usize)>> = std::sync::Mutex::new(Vec::new());

/// How many frames the stack trace keeps (the pairs live in fields of an object,
/// up to 2¹⁶ per object).
const FRAMES_IN_TRACE: usize = 1024;

/// Today's text, without a table or without a Dart frame.
const TRACE_WITHOUT_TABLE: &str = "#0      main (dart:native)\n";

/// The marks of a function record (`emit_native/src/hir.rs`, `marcas_do_rastro`).
const ASYNC_BODY_MARK: u8 = 1;
const HIDDEN_MARK: u8 = 2;
const LISTEN_MARK: u8 = 4;
/// The body belongs to an `async*` generator (with [`ASYNC_BODY_MARK`]): its frame
/// on the stack has already been suspended (the generator's `WasPreviouslySuspended`).
const ASYNC_GENERATOR_MARK: u8 = 8;

/// The states of `_StreamController` and of `_BufferingStreamSubscription`
/// (`stream_controller.dart`, `stream_impl.dart`).
const CONTROLLER_SUBSCRIBED: i64 = 1;
const SUBSCRIPTION_MASK: i64 = 3;
const CONTROLLER_ADD_STREAM: i64 = 8;
const SUBSCRIPTION_WITH_ERROR: i64 = 1 << 5;

/// The kinds of tear-off that the walk recognizes (the `Owner` of the implicit
/// function in the VM): the `_onData` of a `_StreamIterator` and the `_add` of a
/// `_StreamController`.
const ITERATOR_ON_DATA_TEAROFF: i64 = 1;
const CONTROLLER_ADD_TEAROFF: i64 = 2;

/// The entries of the recognized tear-offs: `(input, kind)`.
static TRACE_TEAROFFS: std::sync::Mutex<Vec<(usize, i64)>> = std::sync::Mutex::new(Vec::new());

/// The class id of the frame of an `async` function (`lower/async_sm.rs`,
/// `ASYNC_FRAME_ID`): `[state, completer, body, …]`.
const ASYNC_FRAME_ID: i64 = 0x3FFF_FF00;

/// The states of `_FutureListener` (`future_impl.dart`).
const LISTENER_THEN: i64 = 1;
const LISTENER_CATCH_ERROR: i64 = 2;
const LISTENER_WHEN_COMPLETE: i64 = 8;
const LISTENER_AWAIT: i64 = 16;

/// The maximum distance between the address written by `dartforge_rastro_entrar` (a
/// local of it) and the stack top of the frame that called it.
const STACK_WINDOW: usize = 16 * 1024;

/// The stored pairs of a frame of the awaiter chain (those of a synchronous frame
/// are `(ret, start)`, with the high bits clear).
const PAIR_WAIT: u64 = 1 << 62;
const PAIR_LISTENER: u64 = 1 << 61;

/// A call site, already with absolute addresses.
#[derive(Clone, Copy)]
struct TracePoint {
    label_name: usize,
    rec: usize,
    line_column: u32,
}

/// A function known through the uniform entry of its closures.
#[derive(Clone, Default)]
struct TraceFunction {
    rec: usize,
    token: u32,
    /// The position of the `await` of each state (index `k − 1`).
    waits: Vec<u32>,
    /// The link with the awaiter: `(position, direct, cell)`.
    link: Option<(usize, bool, bool)>,
}

/// The parsed sections.
struct TraceIndex {
    /// The call sites by label; those with the same label, in section order, are
    /// the chain of inlined frames.
    points: Vec<TracePoint>,
    /// The functions by entry, in order.
    functions: Vec<(usize, TraceFunction)>,
}

impl TraceIndex {
    /// The chain of the frame of `ret` (the largest label below it, within the
    /// function that starts at `start`), from the inside out; empty without a site.
    fn chain(&self, ret: u64, start: u64) -> &[TracePoint] {
        let r = ret as usize;
        let k = self.points.partition_point(|p| p.label_name < r);
        let Some(last_one) = k.checked_sub(1) else { return &[] };
        let label_name = self.points[last_one].label_name;
        // The frame's function: the label must be in it. Without the start (the
        // unwinder did not give it), only a nearby label.
        if (start != 0 && label_name < start as usize) || (start == 0 && r - label_name > 1 << 16) {
            return &[];
        }
        let mut first = last_one;
        while first > 0 && self.points[first - 1].label_name == label_name {
            first -= 1;
        }
        &self.points[first..=last_one]
    }

    fn function(&self, input: usize) -> Option<&TraceFunction> {
        self.functions.binary_search_by_key(&input, |(e, _)| *e).ok().map(|i| &self.functions[i].1)
    }
}

/// A frame of the stored stack trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TraceFrame {
    /// The return address and the start of the function.
    Sync(u64, u64),
    /// `<asynchronous suspension>`.
    Gap,
    /// The awaiter in an `await`: the body's entry and the state.
    Wait(u64, i64),
    /// The closure that listens to a `Future`: its entry.
    Listener(u64),
}

fn frame_pair(q: TraceFrame) -> (u64, u64) {
    match q {
        TraceFrame::Sync(r, f) => (r, f),
        TraceFrame::Gap => (0, 0),
        TraceFrame::Wait(e, k) => (PAIR_WAIT | (k as u64 & (PAIR_LISTENER - 1)), e),
        TraceFrame::Listener(e) => (PAIR_LISTENER, e),
    }
}

fn pair_frame((a, b): (u64, u64)) -> TraceFrame {
    if a == 0 && b == 0 {
        TraceFrame::Gap
    } else if a & PAIR_WAIT != 0 {
        TraceFrame::Wait(b, (a & !PAIR_WAIT) as i64)
    } else if a == PAIR_LISTENER {
        TraceFrame::Listener(b)
    } else {
        TraceFrame::Sync(a, b)
    }
}

thread_local! {
    /// The pairs of this thread's last `throw` (see [`frame_pair`]), from the
    /// inside out; empty without a table.
    static THROW_RETURNS: std::cell::RefCell<Vec<(u64, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The stack-trace stack: `(type, value, address)` pushed by
    /// `dartforge_rastro_entrar` — the frame of an `async` body (type 0) or the
    /// `handleValue` listener (type 1) and a machine-stack address right below the
    /// calling frame.
    static TRACE_STACK: std::cell::RefCell<Vec<(i64, i64, usize)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The registration of an image's stack-trace table: the module compiled with
/// `--rastro=simbolico` calls this at startup with the start and the end of the
/// section (`dfpcl`; the `.dfpcl$a`/`$z` sentinels in COFF). Idempotent per
/// section; neither allocates on the collector's heap nor throws.
///
/// # Safety
/// `[start, end)` is the stack-trace section of a loaded image, which stays loaded
/// until the end of the process.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_rastro(start: *const u8, end: *const u8) {
    let (start, end) = (start as usize, end as usize);
    if start == 0 || end <= start {
        return;
    }
    let mut sections = TRACE_SECTIONS.lock().unwrap_or_else(|e| e.into_inner());
    if sections.iter().any(|&(a, _)| a == start) {
        return;
    }
    sections.push((start, end));
    TRACE_GENERATION.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    HAS_TRACE_TABLE.store(true, std::sync::atomic::Ordering::Release);
}

/// Removes the stack-trace section registered with `start` (the JIT object that the
/// session releases, on a reload). Neither allocates on the collector's heap nor throws.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_desregistrar_rastro(start: *const u8) {
    let start = start as usize;
    let mut sections = TRACE_SECTIONS.lock().unwrap_or_else(|e| e.into_inner());
    let before = sections.len();
    sections.retain(|&(a, _)| a != start);
    if sections.len() != before {
        TRACE_GENERATION.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }
}

/// The position of a `dart:async` field that the awaiter chain reads
/// (`"Class.field"`, `n` bytes at `name`). Idempotent; neither allocates on the
/// collector's heap nor throws.
///
/// # Safety
/// `name` points to `n` valid UTF-8 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_campo_do_rastro(class: i64, name: *const u8, n: i64, cur_position: i64) {
    let (Ok(n), Ok(cur_position)) = (usize::try_from(n), usize::try_from(cur_position)) else { return };
    if name.is_null() {
        return;
    }
    // SAFETY: the function's contract.
    let bytes = unsafe { std::slice::from_raw_parts(name, n) };
    let Ok(name) = std::str::from_utf8(bytes) else { return };
    let mut fields = TRACE_FIELDS.lock().unwrap_or_else(|e| e.into_inner());
    if !fields.iter().any(|(c, x, _)| *c == class && x == name) {
        fields.push((class, name.to_string(), cur_position));
    }
}

/// The entry of the tear-off of kind `kind` (the `_onData` of `_StreamIterator`, the
/// `_add` of `_StreamController`) that the image has. Idempotent; neither allocates
/// on the collector's heap nor throws.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_registrar_tearoff_do_rastro(input: *const u8, kind: i64) {
    let input = input as usize;
    if input == 0 {
        return;
    }
    let mut v = TRACE_TEAROFFS.lock().unwrap_or_else(|e| e.into_inner());
    if !v.iter().any(|&(e, _)| e == input) {
        v.push((input, kind));
    }
}

/// Pushes onto the stack-trace stack the frame of an `async` body (`tipo` 0) or the
/// `_FutureListener.handleValue` listener (`tipo` 1) at the function's entry;
/// returns the depth, which the exit passes to [`dartforge_rastro_sair`].
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn dartforge_rastro_entrar(tipo: i64, value: i64) -> i64 {
    let mark = std::hint::black_box(0u8);
    let address = std::hint::black_box(std::ptr::addr_of!(mark)) as usize;
    TRACE_STACK.with(|p| {
        let mut p = p.borrow_mut();
        p.push((tipo, value, address));
        p.len() as i64
    })
}

/// The exit of the function that entered with `depth`: cuts the stack-trace stack
/// there (whatever functions unwound by an exception left on top falls with it).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rastro_sair(depth: i64) {
    let n = usize::try_from(depth.saturating_sub(1)).unwrap_or(0);
    TRACE_STACK.with(|p| p.borrow_mut().truncate(n));
}

/// Some image registered the stack-trace table.
fn has_trace_table() -> bool {
    HAS_TRACE_TABLE.load(std::sync::atomic::Ordering::Acquire)
}

/// The pairs of the stack from here outwards (the synchronous frames and the
/// awaiter chain), with a table; empty without.
fn capture_returns() -> Vec<(u64, u64)> {
    if !has_trace_table() {
        return Vec::new();
    }
    let frames = crate::heap::return_addresses(FRAMES_IN_TRACE);
    let stack = TRACE_STACK.with(|p| p.borrow().clone());
    // With nothing on the stack-trace stack, no frame matches: only the synchronous
    // ones (without building the index at `throw`).
    if stack.is_empty() {
        return frames.into_iter().map(|(r, f, _)| (r, f)).collect();
    }
    let frames = with_trace_index(|ix| heap_without_borrow(|heap| unwind(ix, heap, &frames, &stack)));
    frames.into_iter().map(frame_pair).collect()
}

/// The `throw`: stores the pairs of this stack.
fn store_throw_returns() {
    if !has_trace_table() {
        return;
    }
    let returns = capture_returns();
    THROW_RETURNS.with(|r| *r.borrow_mut() = returns);
}

/// The pairs of this thread's last `throw` (empty without a table).
fn throw_returns() -> Vec<(u64, u64)> {
    THROW_RETURNS.with(|r| r.borrow().clone())
}

/// Forgets the pairs of the last `throw` (the exception was handled).
fn forget_throw_returns() {
    THROW_RETURNS.with(|r| r.borrow_mut().clear());
}

// --- the awaiter chain (`AsyncAwareStackUnwinder`) ---------------------------

/// The awaiter frame under construction (the VM's `AwaiterFrame`): the closure and
/// the next object of the chain (the frame of an `async` body, a `Completer`, a
/// `_Future`).
#[derive(Default)]
struct Waiter {
    closure: Option<i64>,
    next_offset: Option<i64>,
}

/// The class name of `h`, if it is an object.
fn class_of(heap: &crate::heap::Heap, h: i64) -> Option<String> {
    let cid = heap.object_class(h)?;
    CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned())
}

/// Field `field` of object `h` (the position registered for its class).
fn trace_field(heap: &crate::heap::Heap, h: i64, field: &str) -> Option<crate::heap::Field> {
    let cid = heap.object_class(h)?;
    let cur_position = {
        let fields = TRACE_FIELDS.lock().unwrap_or_else(|e| e.into_inner());
        fields.iter().find(|(c, name, _)| *c == cid && name.rsplit('.').next() == Some(field)).map(|(_, _, p)| *p)?
    };
    heap.object(h)?.get(cur_position)
}

/// The reference field (non-null).
fn trace_ref(heap: &crate::heap::Heap, h: i64, field: &str) -> Option<i64> {
    match trace_field(heap, h, field)? {
        (r, true) if r != 0 => Some(r),
        _ => None,
    }
}

/// An `int` stored in a field (raw or boxed).
fn field_int(heap: &crate::heap::Heap, c: crate::heap::Field) -> Option<i64> {
    match c {
        (b, false) => Some(b),
        (r, true) => heap.int_of(r),
    }
}

fn is_async_frame(heap: &crate::heap::Heap, h: i64) -> bool {
    heap.object_class(h) == Some(ASYNC_FRAME_ID)
}

/// The state of the frame of an `async` body (0 before the first `await`).
fn frame_state(heap: &crate::heap::Heap, q: i64) -> i64 {
    heap.object(q).and_then(|o| o.get(0)).and_then(|c| field_int(heap, c)).unwrap_or(0)
}

/// The marks of function record `r`.
#[allow(unsafe_code)]
fn record_marks(r: usize) -> u8 {
    // SAFETY: the record of an entry of a registered section (the marks byte
    // follows the offset to the url).
    unsafe { ((r + 4) as *const u8).read() }
}

/// The link of closure `c` with its awaiter (`TryGetAwaiterLink`): `None` if its
/// function has no link; `Some(None)` if the link is null.
fn closure_link(ix: &TraceIndex, heap: &crate::heap::Heap, c: i64) -> Option<Option<i64>> {
    let clo = heap.closure(c)?;
    let f = ix.function(clo.code as usize)?;
    let value = if record_marks(f.rec) & ASYNC_BODY_MARK != 0 {
        // The body's closure: the frame at position 0 of the environment.
        heap.object(clo.contexto.0)?.get(0)?
    } else {
        let (cur_position, direct, cell) = f.link?;
        let v = if direct { clo.contexto } else { heap.object(clo.contexto.0)?.get(cur_position)? };
        if cell && v.1 { heap.object(v.0).and_then(|o| o.get(0)).unwrap_or((0, false)) } else { v }
    };
    Some((value.1 && value.0 != 0).then_some(value.0))
}

/// `ComputeNextFrameFromAwaiterLink` (with `FollowAwaiterLinks`): follows the
/// closures' links while the link is another closure; the last link that is not a
/// closure becomes the next one.
fn follow_links(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter) {
    let mut link: Option<i64> = None;
    let mut steps = 0;
    while let Some(c) = q.closure {
        link = None;
        let Some(l) = closure_link(ix, heap, c) else { break };
        link = l;
        match l {
            Some(x) if heap.closure(x).is_some() && steps < 64 => {
                q.closure = Some(x);
                steps += 1;
            }
            _ => break,
        }
    }
    if let Some(x) = link
        && heap.closure(x).is_none()
    {
        q.next_offset = Some(x);
    }
}

/// `InitializeAwaiterFrameFromFutureListener`.
fn start_from_listener(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter, listener: i64) {
    if class_of(heap, listener).as_deref() != Some("_FutureListener") {
        *q = Waiter::default();
        return;
    }
    let estado = trace_field(heap, listener, "state").and_then(|c| field_int(heap, c)).unwrap_or(-1);
    let result = if [
        LISTENER_THEN,
        LISTENER_CATCH_ERROR,
        LISTENER_WHEN_COMPLETE,
        LISTENER_THEN | LISTENER_CATCH_ERROR,
        LISTENER_THEN | LISTENER_CATCH_ERROR | LISTENER_AWAIT,
    ]
    .contains(&estado)
    {
        trace_ref(heap, listener, "result")
    } else {
        None
    };
    q.closure = trace_ref(heap, listener, "callback").filter(|&c| heap.closure(c).is_some());
    q.next_offset = result;
    follow_links(ix, heap, q);
}

/// `UnwindFrameToFutureListener`: the first listener of the next one's `_Future`.
fn up_to_listener(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter) {
    let listener = q.next_offset.and_then(|f| trace_ref(heap, f, "_resultOrListeners"));
    match listener {
        Some(l) if class_of(heap, l).as_deref() == Some("_FutureListener") => start_from_listener(ix, heap, q, l),
        _ => *q = Waiter::default(),
    }
}

/// A `bool` stored in a field (raw or boxed).
fn field_bool(heap: &crate::heap::Heap, c: crate::heap::Field) -> Option<bool> {
    match c {
        (b, false) => Some(b != 0),
        (r, true) => heap.bool_of(r),
    }
}

/// The integer field `field` of `h`.
fn trace_int(heap: &crate::heap::Heap, h: i64, field: &str) -> Option<i64> {
    trace_field(heap, h, field).and_then(|c| field_int(heap, c))
}

/// The kind and the receiver of a recognized tear-off.
fn tearoff_of(heap: &crate::heap::Heap, c: i64) -> Option<(i64, Option<i64>)> {
    let clo = heap.closure(c)?;
    let kind = {
        let v = TRACE_TEAROFFS.lock().unwrap_or_else(|e| e.into_inner());
        v.iter().find(|&&(e, _)| e as i64 == clo.code).map(|&(_, k)| k)?
    };
    // The tear-off's environment keeps the receiver at position 0.
    let receiver_obj = if clo.contexto.1 {
        heap.object(clo.contexto.0).and_then(|o| o.get(0)).and_then(|v| (v.1 && v.0 != 0).then_some(v.0))
    } else {
        None
    };
    Some((kind, receiver_obj))
}

/// `FollowAwaiterLinks`: follows the links of `closure` while the link is another
/// closure; returns the last link (which is not a closure), or nothing.
fn follow_links_from(ix: &TraceIndex, heap: &crate::heap::Heap, closure: &mut Option<i64>) -> Option<i64> {
    let mut link: Option<i64> = None;
    let mut steps = 0;
    while let Some(c) = *closure {
        link = None;
        let Some(l) = closure_link(ix, heap, c) else { break };
        link = l;
        match l {
            Some(x) if heap.closure(x).is_some() && steps < 64 => {
                *closure = Some(x);
                steps += 1;
            }
            _ => break,
        }
    }
    link.filter(|&x| heap.closure(x).is_none())
}

/// Does the function of closure `c` have a link with its awaiter (`HasAwaiterLink`)?
fn has_link(ix: &TraceIndex, heap: &crate::heap::Heap, c: Option<i64>) -> bool {
    c.is_some_and(|c| closure_link(ix, heap, c).is_some())
}

/// `UnwindFrameToStreamController`: the controller in `q.next_offset`.
fn up_to_controller(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter) {
    let Some(controller) = q.next_offset else {
        *q = Waiter::default();
        return;
    };
    *q = Waiter::default();
    let estado = trace_int(heap, controller, "_state").unwrap_or(0);
    if estado & SUBSCRIPTION_MASK != CONTROLLER_SUBSCRIBED {
        return;
    }
    let mut subscription = trace_ref(heap, controller, "_varData");
    if estado & CONTROLLER_ADD_STREAM != 0 {
        subscription = subscription.and_then(|s| trace_ref(heap, s, "_varData"));
    }
    let Some(subscription) = subscription else { return };
    let mut closure = trace_ref(heap, subscription, "_onData").filter(|&c| heap.closure(c).is_some());
    let subscription_state = trace_int(heap, subscription, "_state").unwrap_or(0);
    // The tear-off of `_StreamIterator._onData` (the `await for`) and that of
    // `_StreamController._add` (the `yield*`).
    if let Some((kind, receiver_obj)) = closure.and_then(|c| tearoff_of(heap, c)) {
        if kind == ITERATOR_ON_DATA_TEAROFF {
            let Some(iterator) = receiver_obj.filter(|&r| class_of(heap, r).as_deref() == Some("_StreamIterator")) else { return };
            // With `_hasValue`, `_stateData` is the value: do not follow.
            if trace_field(heap, iterator, "_hasValue").and_then(|c| field_bool(heap, c)) == Some(true) {
                return;
            }
            // The `_Future` of `await iterator.moveNext()`.
            if let Some(f) = trace_ref(heap, iterator, "_stateData")
                && class_of(heap, f).as_deref() == Some("_Future")
            {
                q.next_offset = Some(f);
            }
            return;
        }
        if kind == CONTROLLER_ADD_TEAROFF
            && let Some(dest) = receiver_obj
        {
            let target_state = trace_int(heap, dest, "_state").unwrap_or(0);
            if target_state & CONTROLLER_ADD_STREAM != 0 {
                let listener = trace_ref(heap, dest, "_varData")
                    .and_then(|s| trace_ref(heap, s, "addStreamFuture"))
                    .and_then(|f| trace_ref(heap, f, "_resultOrListeners"));
                if let Some(o) = listener
                    && class_of(heap, o).as_deref() == Some("_FutureListener")
                    && trace_int(heap, o, "state") == Some(LISTENER_THEN)
                {
                    let mut handler = trace_ref(heap, o, "callback").filter(|&c| heap.closure(c).is_some());
                    let link = follow_links_from(ix, heap, &mut handler);
                    if let Some(c) = link
                        && class_of(heap, c).as_deref() == Some("_AsyncStarStreamController")
                    {
                        q.closure = trace_ref(heap, c, "asyncStarBody").filter(|&x| heap.closure(x).is_some());
                        return;
                    }
                }
            }
        }
    }
    q.closure = closure;
    let mut sibling_with_link = false;
    let mut with_link = has_link(ix, heap, closure);
    if !with_link {
        // The `onError` or the `onDone` with a link.
        closure = trace_ref(heap, subscription, "_onError").filter(|&c| heap.closure(c).is_some());
        with_link = has_link(ix, heap, closure);
        sibling_with_link = with_link;
    }
    if !with_link {
        closure = trace_ref(heap, subscription, "_onDone").filter(|&c| heap.closure(c).is_some());
        with_link = has_link(ix, heap, closure);
        sibling_with_link = with_link;
    }
    let has_error_handler = subscription_state & SUBSCRIPTION_WITH_ERROR != 0;
    let mut object = None;
    if has_error_handler || sibling_with_link {
        object = follow_links_from(ix, heap, &mut closure);
    }
    if sibling_with_link {
        let class = object.and_then(|o| class_of(heap, o));
        if matches!(class.as_deref(), Some("_AsyncStarStreamController" | "_SyncStreamController")) {
            q.closure = closure;
        } else {
            q.next_offset = object;
        }
    }
}

/// `UnwindAwaiterFrame`: from the suspended frame to its `Future` (through the
/// `Completer`), and from the `Future` to the listener.
fn unwind_waiter(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter) {
    let mut next_offset = q.next_offset;
    if let Some(p) = next_offset
        && is_async_frame(heap, p)
    {
        // The frame's `completer` (position 1); in an `async*` generator, the
        // controller (the VM's `SuspendState` `function_data`).
        next_offset = heap.object(p).and_then(|o| o.get(1)).and_then(|c| (c.1 && c.0 != 0).then_some(c.0));
    } else if let Some(p) = next_offset {
        match class_of(heap, p).as_deref() {
            Some("_SyncCompleter" | "_AsyncCompleter") => next_offset = trace_ref(heap, p, "future"),
            _ => {}
        }
    }
    if let Some(p) = next_offset
        && class_of(heap, p).as_deref() == Some("_AsyncAwaitCompleter")
    {
        next_offset = trace_ref(heap, p, "_future");
    }
    if let Some(p) = next_offset
        && class_of(heap, p).as_deref() == Some("_AsyncStarStreamController")
    {
        next_offset = trace_ref(heap, p, "controller");
    }
    match next_offset.and_then(|p| class_of(heap, p)).as_deref() {
        Some("_Future") => {
            q.next_offset = next_offset;
            up_to_listener(ix, heap, q);
        }
        Some("_SyncStreamController") => {
            q.next_offset = next_offset;
            up_to_controller(ix, heap, q);
            follow_links(ix, heap, q);
        }
        _ => *q = Waiter::default(),
    }
}

/// `UnwindToAwaiter`.
fn up_to_waiter(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter) {
    let mut steps = 0;
    loop {
        unwind_waiter(ix, heap, q);
        steps += 1;
        if q.closure.is_some() || q.next_offset.is_none() || steps > 1024 {
            break;
        }
    }
}

/// The stack-trace stack entry of the machine frame whose top is `sp`: the highest
/// one still free, of `tipo`, written right below `sp`. The ones above it are left
/// behind (from deeper frames already matched, or left by unwound frames).
fn match_at(stack: &[(i64, i64, usize)], top: &mut usize, tipo: i64, sp: u64) -> Option<i64> {
    let sp = sp as usize;
    if sp == 0 {
        return None;
    }
    for i in (0..*top).rev() {
        let (t, v, address) = stack[i];
        if t == tipo && address < sp && sp - address < STACK_WINDOW {
            *top = i;
            return Some(v);
        }
    }
    None
}

/// The walk of `AsyncAwareStackUnwinder::Unwind`: the synchronous frames up to the
/// first one with an awaiter, and then the awaiter chain.
fn unwind(ix: &TraceIndex, heap: &crate::heap::Heap, frames: &[(u64, u64, u64)], stack: &[(i64, i64, usize)]) -> Vec<TraceFrame> {
    let mut out = Vec::with_capacity(frames.len() + 8);
    let mut top = stack.len();
    let mut q = Waiter::default();
    'sincronos: for &(ret, start, sp) in frames {
        out.push(TraceFrame::Sync(ret, start));
        for p in ix.chain(ret, start) {
            let marks = record_marks(p.rec);
            if marks & ASYNC_BODY_MARK != 0 {
                // `InitializeAwaiterFrameFromSuspendState`: the resumed body
                // (already suspended once; that of an `async*` generator always)
                // goes on through the chain.
                if let Some(frame) = match_at(stack, &mut top, 0, sp)
                    && (marks & ASYNC_GENERATOR_MARK != 0 || frame_state(heap, frame) != 0)
                {
                    q = Waiter { closure: None, next_offset: Some(frame) };
                    up_to_waiter(ix, heap, &mut q);
                }
            } else if marks & LISTEN_MARK != 0
                && let Some(listener) = match_at(stack, &mut top, 1, sp)
            {
                start_from_listener(ix, heap, &mut q, listener);
                up_to_waiter(ix, heap, &mut q);
            }
            if q.closure.is_some() {
                break 'sincronos;
            }
        }
    }
    let mut some_one = false;
    let mut steps = 0;
    while let Some(c) = q.closure {
        steps += 1;
        if steps > FRAMES_IN_TRACE {
            break;
        }
        let input = heap.closure(c).map_or(0, |clo| clo.code as u64);
        some_one = true;
        match q.next_offset {
            Some(p) if is_async_frame(heap, p) => {
                let k = frame_state(heap, p);
                // `pc == 0`: the body has already been resumed (it is on the stack).
                if k > 0 {
                    out.push(TraceFrame::Gap);
                    out.push(TraceFrame::Wait(input, k));
                }
            }
            _ => {
                out.push(TraceFrame::Gap);
                out.push(TraceFrame::Listener(input));
            }
        }
        up_to_waiter(ix, heap, &mut q);
    }
    if some_one {
        out.push(TraceFrame::Gap);
    }
    out
}

// --- the sections and the text -----------------------------------------------

/// Reads the entries of `[start, end)`. An all-zero entry is a sentinel or padding.
///
/// # Safety
/// `[start, end)` is a mapped stack-trace section.
#[allow(unsafe_code)]
unsafe fn read_trace_section(start: usize, end: usize, points: &mut Vec<TracePoint>, functions: &mut Vec<(usize, TraceFunction)>) {
    let mut p = (start + 3) & !3;
    while p + 12 <= end {
        // The compact form of a converted object (`rastro_compacto.rs`).
        // SAFETY: `p .. p + 4` is within the section.
        if unsafe { std::slice::from_raw_parts(p as *const u8, 4) } == b"DFPC" {
            // SAFETY: the blob starts at `p` and stays within the section.
            match unsafe { read_trace_blob(p, end, points, functions) } {
                Some(size) => {
                    p += (size + 3) & !3;
                    continue;
                }
                None => return,
            }
        }
        // SAFETY: `p .. p + 12` is within the section.
        let (label_name, rec, word) = unsafe {
            (
                (p as *const i32).read_unaligned(),
                ((p + 4) as *const i32).read_unaligned(),
                ((p + 8) as *const u32).read_unaligned(),
            )
        };
        if label_name != 0 || rec != 0 || word != 0 {
            let rot = p.wrapping_add_signed(label_name as isize);
            let kind = (rec & 3) as u32;
            let reg = (p + 4).wrapping_add_signed((rec - kind as i32) as isize);
            match kind {
                0 => points.push(TracePoint { label_name: rot, rec: reg, line_column: word }),
                _ => {
                    if functions.last().is_none_or(|(e, _)| *e != rot) {
                        functions.push((rot, TraceFunction::default()));
                    }
                    let f = &mut functions.last_mut().expect("the function just pushed").1;
                    match kind {
                        1 => {
                            f.rec = reg;
                            f.token = word;
                        }
                        2 => f.waits.push(word),
                        _ => f.link = Some(((word >> 2) as usize, word & 2 != 0, word & 1 != 0)),
                    }
                }
            }
        }
        p += 12;
    }
}

/// A LEB128 varint at `*p`, without going past `end`.
///
/// # Safety
/// `[*p, end)` is mapped.
#[allow(unsafe_code)]
unsafe fn trace_varint(p: &mut usize, end: usize) -> Option<u64> {
    let mut r = 0u64;
    let mut s = 0u32;
    loop {
        if *p >= end {
            return None;
        }
        // SAFETY: `*p < end`.
        let b = unsafe { (*p as *const u8).read() };
        *p += 1;
        if s < 64 {
            r |= u64::from(b & 0x7f) << s;
        }
        s += 7;
        if b & 0x80 == 0 {
            return Some(r);
        }
    }
}

/// Reads the DFPC blob at `start` (the format of `rastro_compacto.rs`): the
/// function index and the records, relocated relative to their own field (in COFF,
/// to the next byte), and each function's stream. Returns the blob's size; `None`
/// for a malformed blob (the rest of the section is ignored).
///
/// # Safety
/// `[start, end)` is a mapped stack-trace section.
#[allow(unsafe_code)]
unsafe fn read_trace_blob(start: usize, end: usize, points: &mut Vec<TracePoint>, functions: &mut Vec<(usize, TraceFunction)>) -> Option<usize> {
    // SAFETY: the fields read stay within `[start, end)` (checked).
    unsafe {
        let u32_at = |q: usize| (q as *const u32).read_unaligned();
        if start + 20 > end || (start as *const u8).add(4).read() != 1 {
            return None;
        }
        let shape = (start as *const u8).add(5).read();
        let size = u32_at(start + 8) as usize;
        let nf = u32_at(start + 12) as usize;
        let nr = u32_at(start + 16) as usize;
        if size < 20 || start + size > end || 20 + 8 * nf + 4 * nr > size {
            return None;
        }
        let blob_end = start + size;
        let address = |field: usize| {
            let v = (field as *const i32).read_unaligned() as isize;
            if shape == 2 { (field + 4).wrapping_add_signed(v) } else { field.wrapping_add_signed(v) }
        };
        let records_base = start + 20 + 8 * nf;
        let records: Vec<usize> = (0..nr).map(|k| address(records_base + 4 * k)).collect();
        for k in 0..nf {
            let field = start + 20 + 8 * k;
            let function = address(field);
            let mut q = start + u32_at(field + 4) as usize;
            let n = trace_varint(&mut q, blob_end)?;
            let mut offset = 0usize;
            for _ in 0..n {
                offset += trace_varint(&mut q, blob_end)? as usize;
                let rk = trace_varint(&mut q, blob_end)?;
                let word = trace_varint(&mut q, blob_end)? as u32;
                let rot = function + offset;
                let kind = rk & 3;
                let reg = records.get((rk >> 2) as usize).copied().unwrap_or(0);
                match kind {
                    0 => points.push(TracePoint { label_name: rot, rec: reg, line_column: word }),
                    _ => {
                        if functions.last().is_none_or(|(e, _)| *e != rot) {
                            functions.push((rot, TraceFunction::default()));
                        }
                        let f = &mut functions.last_mut().expect("the function just pushed").1;
                        match kind {
                            1 => {
                                f.rec = reg;
                                f.token = word;
                            }
                            2 => f.waits.push(word),
                            _ => f.link = Some(((word >> 2) as usize, word & 2 != 0, word & 1 != 0)),
                        }
                    }
                }
            }
        }
        Some(size)
    }
}

/// Calls `f` with the index of all the registered sections.
#[allow(unsafe_code)]
fn with_trace_index<R>(f: impl FnOnce(&TraceIndex) -> R) -> R {
    let (sections, generation) = {
        let s = TRACE_SECTIONS.lock().unwrap_or_else(|e| e.into_inner());
        (s.clone(), TRACE_GENERATION.load(std::sync::atomic::Ordering::Acquire))
    };
    {
        let ix = TRACE_INDEX.read().unwrap_or_else(|e| e.into_inner());
        if ix.0 == generation {
            return f(&ix.1);
        }
    }
    let mut points = Vec::new();
    let mut functions = Vec::new();
    for &(start, end) in &sections {
        // SAFETY: sections registered by `dartforge_registrar_rastro`.
        unsafe { read_trace_section(start, end, &mut points, &mut functions) };
    }
    // Stable: those with the same label stay in section order (from the inside out).
    points.sort_by_key(|p| p.label_name);
    functions.sort_by_key(|(e, _)| *e);
    functions.dedup_by_key(|(e, _)| *e);
    let mut ix = TRACE_INDEX.write().unwrap_or_else(|e| e.into_inner());
    *ix = (generation, TraceIndex { points, functions });
    f(&ix.1)
}

/// The name, the url and the marks of the function record at `r`.
///
/// # Safety
/// `r` is a function record of a registered section.
#[allow(unsafe_code)]
unsafe fn trace_record(r: usize) -> (String, String, u8) {
    // SAFETY: the record format (`llvm/rastro.rs`): `i32` up to the url, the marks
    // byte, the name; the strings NUL-terminated.
    unsafe {
        let until_url = (r as *const i32).read_unaligned();
        let url = std::ffi::CStr::from_ptr(r.wrapping_add_signed(until_url as isize) as *const std::ffi::c_char);
        let marks = ((r + 4) as *const u8).read();
        let name = std::ffi::CStr::from_ptr((r + 5) as *const std::ffi::c_char);
        (name.to_string_lossy().into_owned(), url.to_string_lossy().into_owned(), marks)
    }
}

/// The stack-trace text of the pairs in the VM's format: `#<index>` left-aligned in
/// 6 columns, the name and `(url:line:column)`, the line and the column only when
/// known; the inlined frames of a site, from the inside out; the hidden ones, left
/// out; and `<asynchronous suspension>` between the stretches of the awaiter chain,
/// once per gap. Without any frame, today's text.
fn symbolize_returns(pair_list: &[(u64, u64)]) -> String {
    with_trace_index(|ix| symbolize_with(ix, pair_list))
}

fn escrever_quadro(text: &mut String, n: usize, name: &str, url: &str, line_column: u32) {
    use std::fmt::Write as _;
    let (line, column) = (line_column >> 12, line_column & 0xFFF);
    let _ = write!(text, "#{n:<6} {name} ({url}");
    if line > 0 {
        let _ = write!(text, ":{line}");
        if column > 0 {
            let _ = write!(text, ":{column}");
        }
    }
    text.push_str(")\n");
}

/// [`symbolize_returns`] with the given index.
#[allow(unsafe_code)]
fn symbolize_with(ix: &TraceIndex, pair_list: &[(u64, u64)]) -> String {
    let mut text = String::new();
    let mut n = 0usize;
    let mut in_gap = false;
    let mut some_frame = false;
    for &pair in pair_list {
        match pair_frame(pair) {
            TraceFrame::Sync(ret, start) => {
                for p in ix.chain(ret, start) {
                    // SAFETY: the record of a section entry.
                    let (name, url, marks) = unsafe { trace_record(p.rec) };
                    if marks & HIDDEN_MARK != 0 {
                        continue;
                    }
                    escrever_quadro(&mut text, n, &name, &url, p.line_column);
                    n += 1;
                    in_gap = false;
                    some_frame = true;
                }
            }
            TraceFrame::Gap => {
                if !in_gap {
                    text.push_str("<asynchronous suspension>\n");
                }
                in_gap = true;
            }
            TraceFrame::Wait(input, k) => {
                let Some(f) = ix.function(input as usize) else { continue };
                let cur_position = usize::try_from(k).ok().and_then(|k| k.checked_sub(1)).and_then(|i| f.waits.get(i)).copied().unwrap_or(0);
                // SAFETY: the record of a section entry.
                let (name, url, _) = unsafe { trace_record(f.rec) };
                escrever_quadro(&mut text, n, &name, &url, cur_position);
                n += 1;
                in_gap = false;
                some_frame = true;
            }
            TraceFrame::Listener(input) => {
                let Some(f) = ix.function(input as usize) else { continue };
                // SAFETY: the record of a section entry.
                let (name, url, marks) = unsafe { trace_record(f.rec) };
                if marks & HIDDEN_MARK != 0 {
                    continue;
                }
                escrever_quadro(&mut text, n, &name, &url, f.token);
                n += 1;
                in_gap = false;
                some_frame = true;
            }
        }
    }
    if !some_frame {
        return TRACE_WITHOUT_TABLE.to_string();
    }
    text
}

/// The pairs stored in a `StackTrace` of the symbolic stack trace: field 0 null (the
/// text, built on the first request), field 1 the number of pairs and the pairs
/// after it, all raw. Empty in a text `StackTrace`.
fn object_returns(heap: &crate::heap::Heap, h: i64) -> Vec<(u64, u64)> {
    let Some(o) = heap.object(h) else { return Vec::new() };
    let Some((n, false)) = o.get(1) else { return Vec::new() };
    let n = usize::try_from(n).unwrap_or(0);
    let mut returns = Vec::with_capacity(n);
    for i in 0..n {
        match (o.get(2 + 2 * i), o.get(3 + 2 * i)) {
            (Some((r, false)), Some((f, false))) => returns.push((r as u64, f as u64)),
            _ => break,
        }
    }
    returns
}

/// A `StackTrace` with the pairs `returns` (see [`object_returns`]); one
/// allocation, with no reference to root.
fn allocate_return_trace(returns: &[(u64, u64)]) -> i64 {
    let cid = stack_trace_class_id();
    let mut fields: Vec<crate::heap::Field> = Vec::with_capacity(2 + 2 * returns.len());
    fields.push((0, true));
    fields.push((returns.len() as i64, false));
    for &(r, f) in returns {
        fields.push((r as i64, false));
        fields.push((f as i64, false));
    }
    HEAP.with(|h| h.borrow_mut().new_object(cid, &fields))
}

/// The text of `StackTrace` `h` as a `String` (the uncaught exception, the
/// runtime's description): that of field 0, or the symbolized pairs. `None`: `h` is
/// not an object.
fn trace_text(h: i64) -> Option<String> {
    let (text, returns) = HEAP.with(|heap| {
        let heap = heap.borrow();
        let o = heap.object(h)?;
        Some(match o.first() {
            Some((t, true)) if t != 0 => (heap.text(t).map(|t| t.para_string()), Vec::new()),
            _ => (None, object_returns(&heap, h)),
        })
    })?;
    Some(text.unwrap_or_else(|| symbolize_returns(&returns)))
}

/// `_StackTrace.toString` (the VM has it in C++): the text of field 0, or the pairs
/// symbolized now and stored in field 0 for the next request.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rastro_texto(h: i64) -> i64 {
    let (ready_flag, returns) = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.object(h).and_then(|o| o.first()) {
            Some((t, true)) if t != 0 => (t, Vec::new()),
            _ => (0, object_returns(&heap, h)),
        }
    });
    if ready_flag != 0 {
        return ready_flag;
    }
    let text = if returns.is_empty() { TRACE_WITHOUT_TABLE.to_string() } else { symbolize_returns(&returns) };
    let t = with_roots(&[h], || error_text(&text));
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if heap.object(h).is_some_and(|o| !o.is_empty()) {
            heap.set_field(h, 0, t, true);
        }
    });
    t
}

#[cfg(test)]
mod tests_trace {
    use super::*;

    /// The records, the code and the section of a test in a single allocation. The
    /// section's fields are `i32` offsets, as between the sections of a binary;
    /// separate heap allocations do not have that guarantee (on macOS arm64
    /// `malloc` puts small blocks in regions hundreds of GiB apart), and the
    /// truncated offset would point outside.
    struct Area {
        rest: &'static mut [u8],
    }

    impl Area {
        fn new_area() -> Self {
            Area { rest: Box::leak(vec![0u8; 4096].into_boxed_slice()) }
        }

        /// `n` zeroed bytes of the area, aligned to 16, alive until the end of the
        /// process.
        fn reserve_space(&mut self, n: usize) -> &'static mut [u8] {
            let rest = std::mem::take(&mut self.rest);
            let jump = rest.as_ptr().align_offset(16);
            let (block, rest) = rest[jump..].split_at_mut(n);
            self.rest = rest;
            block
        }
    }

    /// The strings and the records of a test: `(base, rec of each name)`. The url at
    /// 0; each record has the offset to it, the marks byte and the name.
    fn records(area: &mut Area, names: &[(&str, u8)]) -> Vec<usize> {
        let data = area.reserve_space(16 + 32 * names.len());
        data[..15].copy_from_slice(b"file:///a.dart\0");
        let base = data.as_ptr() as usize;
        let mut out = Vec::new();
        for (i, (name, marks)) in names.iter().enumerate() {
            let r = 16 + 32 * i;
            data[r..r + 4].copy_from_slice(&(-(r as i32)).to_le_bytes());
            data[r + 4] = *marks;
            data[r + 5..r + 5 + name.len()].copy_from_slice(name.as_bytes());
            out.push(base + r);
        }
        out
    }

    /// A hand-built section: `(kind, label, rec, word)`, with a zeroed sentinel at
    /// the start.
    fn section(area: &mut Area, entries: &[(u32, usize, usize, u32)]) -> (usize, usize) {
        let section = area.reserve_space(12 * (entries.len() + 1));
        let s = section.as_ptr() as usize;
        for (k, &(kind, label_name, rec, word)) in entries.iter().enumerate() {
            let k = k + 1;
            let e = s + 12 * k;
            section[12 * k..12 * k + 4].copy_from_slice(&((label_name as i64 - e as i64) as i32).to_le_bytes());
            let field = if kind == 3 { 3 } else { (rec as i64 - (e + 4) as i64) as i32 + kind as i32 };
            section[12 * k + 4..12 * k + 8].copy_from_slice(&field.to_le_bytes());
            section[12 * k + 8..12 * k + 12].copy_from_slice(&word.to_le_bytes());
        }
        (s, s + section.len())
    }

    fn ix(area: &mut Area, entries: &[(u32, usize, usize, u32)]) -> TraceIndex {
        let (s, f) = section(area, entries);
        let (mut points, mut functions) = (Vec::new(), Vec::new());
        // SAFETY: the section above, alive until the end of the process.
        unsafe { read_trace_section(s, f, &mut points, &mut functions) };
        points.sort_by_key(|p| p.label_name);
        functions.sort_by_key(|(e, _)| *e);
        TraceIndex { points, functions }
    }

    /// Each return address finds its function's label, and one that does not find
    /// it is left out; the inlined frames come out from the inside out; the hidden
    /// ones do not.
    #[test]
    fn symbolizes_in_vm_format() {
        let mut area = Area::new_area();
        let r = records(&mut area, &[("main", 0), ("f", 0), ("g", 0), ("stub", HIDDEN_MARK)]);
        let code = area.reserve_space(200);
        let c = code.as_ptr() as usize;
        let ind = ix(&mut area, &[
            (0, c + 50, r[1], 7 << 12),
            (0, c + 10, r[0], 3 << 12 | 5),
            // A site with `g` inlined into `main` (from the inside out).
            (0, c + 30, r[2], 9 << 12 | 2),
            (0, c + 30, r[0], 4 << 12 | 1),
            (0, c + 120, r[3], 1 << 12),
        ]);
        assert_eq!(ind.points.len(), 5, "the sentinel is not an entry");
        let c = c as u64;
        let text = symbolize_with(&ind, &[(c + 5, c), (c + 20, c), (c + 35, c), (c + 60, c + 55), (c + 60, c + 40), (c + 125, c + 100)]);
        assert_eq!(
            text,
            "#0      main (file:///a.dart:3:5)\n#1      g (file:///a.dart:9:2)\n#2      main (file:///a.dart:4:1)\n#3      f (file:///a.dart:7)\n"
        );
        assert_eq!(symbolize_with(&ind, &[(c + 5, c)]), TRACE_WITHOUT_TABLE);
    }

    /// The awaiter chain: the gap before each frame and at the end, only once
    /// between stretches; the awaiter's frame at the position of the state's
    /// `await`; the listener at the token.
    #[test]
    fn writes_waiter_chain() {
        let mut area = Area::new_area();
        let r = records(&mut area, &[("f", ASYNC_BODY_MARK), ("main", ASYNC_BODY_MARK), ("main.<anonymous closure>", 0)]);
        let code = area.reserve_space(200);
        let c = code.as_ptr() as usize;
        let ind = ix(&mut area, &[
            (0, c + 10, r[0], 2 << 12 | 3),
            (1, c + 100, r[1], 1 << 12 | 6),
            (2, c + 100, r[1], 5 << 12 | 9),
            (2, c + 100, r[1], 6 << 12 | 9),
            (1, c + 150, r[2], 8 << 12 | 20),
            (3, c + 150, 0, 1 << 2 | 1),
        ]);
        assert_eq!(ind.function(c + 100).map(|f| f.waits.len()), Some(2));
        assert_eq!(ind.function(c + 150).and_then(|f| f.link), Some((1, false, true)));
        let c = c as u64;
        let pair_list: Vec<(u64, u64)> = [
            TraceFrame::Sync(c + 15, c),
            TraceFrame::Gap,
            TraceFrame::Wait(c + 100, 2),
            TraceFrame::Gap,
            TraceFrame::Gap,
            TraceFrame::Listener(c + 150),
            TraceFrame::Gap,
        ]
        .into_iter()
        .map(frame_pair)
        .collect();
        assert_eq!(
            symbolize_with(&ind, &pair_list),
            "#0      f (file:///a.dart:2:3)\n<asynchronous suspension>\n#1      main (file:///a.dart:6:9)\n<asynchronous suspension>\n#2      main.<anonymous closure> (file:///a.dart:8:20)\n<asynchronous suspension>\n"
        );
        for q in [TraceFrame::Gap, TraceFrame::Wait(77, 3), TraceFrame::Listener(78), TraceFrame::Sync(5, 1)] {
            assert_eq!(pair_frame(frame_pair(q)), q);
        }
    }

    /// A stack-trace stack entry matches the frame whose top is right above it; that
    /// of an unwound frame is left behind.
    #[test]
    fn matches_trace_stack_by_frame_top() {
        let stack = [(0, 11, 9_000usize), (1, 22, 5_000), (0, 33, 4_000)];
        let mut top = stack.len();
        // The deepest frame (top at 5 100): the entry at 4 000 belongs to a frame
        // deeper than it, already unwound, and does not match the type.
        assert_eq!(match_at(&stack, &mut top, 1, 5_100), Some(22));
        assert_eq!(top, 1);
        assert_eq!(match_at(&stack, &mut top, 0, 9_050), Some(11));
        assert_eq!(match_at(&stack, &mut top, 0, 9_050), None);
        assert_eq!(match_at(&stack, &mut 3, 0, 0), None);
    }
}
