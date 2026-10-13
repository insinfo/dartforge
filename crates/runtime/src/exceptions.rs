// Native runtime: the pending exception, stack traces and the SDK's error classes.

/// A new heap string with the text `s` (`text.rs`).
fn error_text(s: &str) -> i64 {
    HEAP.with(|h| h.borrow_mut().allocate_str(s))
}

/// The value of a pending exception in a `Ref` position (boxes the thrown scalar:
/// `throw 1` keeps the `int` unboxed).
fn exception_as_ref(v: crate::heap::Value) -> i64 {
    HEAP.with(|h| h.borrow_mut().to_ref(v))
}

/// The exception thrown through generated code's flat `(bits, tag)` ABI (1 `int`,
/// 2 `bool`, 3 `Ref`, 4 `double`).
fn abi_exception(bits: i64, tag: u8) -> crate::heap::Value {
    use crate::heap::Value;
    match tag {
        1 => Value::Int(bits),
        2 => Value::Bool(bits != 0),
        3 => Value::Ref(bits),
        4 => Value::Double(f64::from_bits(bits as u64)),
        _ => panic!("invalid value tag"),
    }
}

/// The flat-ABI tag of a stored exception.
fn exception_tag(v: crate::heap::Value) -> u8 {
    use crate::heap::Value;
    match v {
        Value::Int(_) => 1,
        Value::Bool(_) => 2,
        Value::Ref(_) => 3,
        Value::Double(_) => 4,
    }
}

/// The flat-ABI bits of a stored exception.
fn exception_bits(v: crate::heap::Value) -> i64 {
    use crate::heap::Value;
    match v {
        Value::Int(i) | Value::Ref(i) => i,
        Value::Bool(b) => i64::from(b),
        Value::Double(d) => d.to_bits() as i64,
    }
}

/// The `Ref` of a stored exception that is a reference (`None` for a scalar).
fn exception_ref(v: crate::heap::Value) -> Option<i64> {
    match v {
        crate::heap::Value::Ref(r) => Some(r),
        _ => None,
    }
}

/// The message of the VM's `TypeError` for `x!` on null.
const NULL_CHECK_MESSAGE: &str = "Null check operator used on a null value";

/// `x!` on null: the VM's `TypeError`, catchable, with its message.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_null_check_error_new() -> i64 {
    if let Some(e) = source_error_with_text("_dartforgeErroDeTipo", NULL_CHECK_MESSAGE) {
        return e;
    }
    let m = error_text(NULL_CHECK_MESSAGE);
    with_roots(&[m], || allocate_error_with_trace(1011, vec![(m, true)]))
}

/// The text of the current exception's stack trace (the `StackTrace` of the
/// throw), for the uncaught exception; empty without a stack trace.
pub fn exception_trace_text() -> String {
    let Some(h) = CURRENT_STACK_TRACE.with(|slot| *slot.borrow()) else {
        // §13.14: with no `StackTrace` requested, the addresses of the throw (empty
        // without the stack trace table).
        let returns = throw_returns();
        return if returns.is_empty() { String::new() } else { symbolize_returns(&returns) };
    };
    trace_text(h).unwrap_or_default()
}

fn stack_trace_class_id() -> i64 {
    CLASS_NAMES.with(|map| {
        let map = map.borrow();
        map.iter().find(|(_, name)| *name == "_StackTrace")
            .or_else(|| map.iter().find(|(_, name)| *name == "StackTrace"))
            .map(|(&id, _)| id)
    }).unwrap_or(1006)
}

/// A `StackTrace` object with the given text (two allocations, the first rooted
/// during the second).
fn allocate_stack_trace(text: &str) -> i64 {
    let trace_str = error_text(text);
    let cid = stack_trace_class_id();
    with_roots(&[trace_str], || {
        HEAP.with(|h| h.borrow_mut().new_object(cid, &[(trace_str, true)]))
    })
}

/// An error object with the current stack trace in its last field (`st` rooted
/// while the object is allocated).
fn allocate_error_with_trace(class_id: i64, mut fields: Vec<(i64, bool)>) -> i64 {
    let roots: Vec<i64> = fields.iter().filter(|(_, r)| *r).map(|(b, _)| *b).collect();
    with_roots(&roots, || {
        let st = dartforge_stack_trace_get();
        fields.push((st, true));
        with_roots(&[st], || HEAP.with(|h| h.borrow_mut().new_object(class_id, &fields)))
    })
}

/// An allocated, rooted message, and the error with it and the stack trace.
fn allocate_error_with_message(class_id: i64, message: &str, before: Vec<(i64, bool)>, after: Vec<(i64, bool)>) -> i64 {
    let msg = error_text(message);
    with_roots(&[msg], || {
        let mut fields = before;
        fields.push((msg, true));
        fields.extend(after);
        allocate_error_with_trace(class_id, fields)
    })
}

thread_local! {
    static CURRENT_STACK_TRACE: RefCell<Option<i64>> = RefCell::new(None);
}

/// Returns a non-empty StackTrace object managed on the heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_stack_trace_get() -> i64 {
    if let Some(h) = CURRENT_STACK_TRACE.with(|slot| *slot.borrow()) {
        return h;
    }
    // §13.14: the addresses of the last `throw` become the current stack trace
    // (the error's `_stackTrace` and `catch (e, s)` see the same object), the text
    // built only on the first request.
    let returns = throw_returns();
    if !returns.is_empty() {
        let st = allocate_return_trace(&returns);
        CURRENT_STACK_TRACE.with(|slot| *slot.borrow_mut() = Some(st));
        HEAP.with(|h| h.borrow_mut().set_runtime_root(1, st));
        return st;
    }
    allocate_stack_trace(TRACE_WITHOUT_TABLE)
}

/// Native of the VM's static getter `StackTrace.current`: with the stack trace
/// table, the addresses of this stack (§13.14).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_StackTrace_current() -> i64 {
    let returns = capture_returns();
    if !returns.is_empty() {
        return allocate_return_trace(&returns);
    }
    dartforge_stack_trace_get()
}

/// Throws an exception associating an explicit stack trace.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_throw_with_stack_trace(bits: i64, tag: u8, st_handle: i64) {
    CURRENT_STACK_TRACE.with(|slot| *slot.borrow_mut() = Some(st_handle));
    HEAP.with(|h| h.borrow_mut().set_runtime_root(1, st_handle));
    dartforge_exception_throw(bits, tag);
}

/// The VM native `Error._throw`: installs the explicit stack trace and hands the
/// exception to generated code's pending-exception protocol. The return is
/// unreachable in Dart (`Never`), but takes a `Ref` in the ABI of the SDK from source.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Error_throwWithStackTrace(error: i64, trace: i64) -> i64 {
    dartforge_throw_with_stack_trace(error, 3, trace);
    0
}

/// Pending exception of the portable `throw`/`try`/`catch` scheme.
///
/// Instead of native unwinding (`landingpad`/C++ personality), which would require
/// aligning the Rust runtime with Clang's exception ABI on each platform, the LLVM
/// emitter checks `dartforge_exception_pending` after each call and branches to
/// the handler. The payload is a value with an explicit tag: 1 = int, 2 = bool,
/// 3 = live managed reference, 4 = double. `throw null` is a compile-time error in
/// Dart 3.6.2 and never gets here.
///
/// The value is a `heap::Value` (outside the heap): the thrown scalar stays unboxed;
/// `core.rs` reads it at the end of the program.
thread_local! {
    static EXCEPTION: RefCell<Option<crate::heap::Value>> = RefCell::new(None);
    /// The isolate is being unwound to terminate (`Isolate.exit`, the VM's
    /// `UnwindError`): the pending exception cannot be caught, no `catch` receives
    /// it and it does not leave the pending state until the event loop.
    static UNWINDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Stores the pending exception and its mirror in the thread context (which
/// generated code reads, `Context::pending`).
fn set_exception(v: Option<crate::heap::Value>) {
    CONTEXT.with(|c| c.pending.set(u8::from(v.is_some())));
    EXCEPTION.with(|slot| *slot.borrow_mut() = v);
}

/// Takes the pending exception (and clears the mirror).
fn take_exception() -> Option<crate::heap::Value> {
    CONTEXT.with(|c| c.pending.set(0));
    EXCEPTION.with(|slot| slot.borrow_mut().take())
}

/// Starts unwinding the isolate: a pending exception that no `catch` receives, up
/// to the event loop (`isolates.rs`).
fn begin_unwind() {
    UNWINDING.with(|d| d.set(true));
    set_exception(Some(crate::heap::Value::Ref(0)));
    HEAP.with(|h| h.borrow_mut().set_runtime_root(0, 0));
}

/// Whether the isolate is being unwound to terminate.
fn unwinding_now() -> bool {
    UNWINDING.with(|d| d.get())
}

/// Whether the pending exception can be caught by a `catch` (the generated handler
/// asks before the `on T` tests).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_catchable() -> u8 {
    u8::from(!unwinding_now())
}

/// The pending exception as a reference (the value of the `catch` variable).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_peek_ref() -> i64 {
    match EXCEPTION.with(|slot| *slot.borrow()) {
        Some(v) => exception_as_ref(v),
        None => 0,
    }
}

fn allocate_state_error(message: &str) -> i64 {
    allocate_error_with_message(1002, message, Vec::new(), Vec::new()) // StateError
}

fn allocate_range_error(message: &str) -> i64 {
    allocate_error_with_message(1004, message, Vec::new(), Vec::new()) // RangeError
}

/// Registers the pending exception; references must be alive and rooted.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_throw(bits: i64, tag: u8) {
    // §13.14: the return addresses of the throw, before any allocation (the
    // explicit stack trace of `Error.throwWithStackTrace` is already the current
    // stack trace).
    if CURRENT_STACK_TRACE.with(|slot| slot.borrow().is_none()) {
        store_throw_returns();
    }
    let value = abi_exception(bits, tag);
    let reference = exception_ref(value).unwrap_or(0);
    if reference != 0 {
        // The SDK from source reserves the first field of `Error` for
        // `_stackTrace`; subclasses keep that prefix in the R7 layout. The
        // internal errors (ids 1000–1012) keep their own indices. The stack trace
        // is allocated WITHOUT an open mutable borrow of the heap (G6).
        let sdk_error = CLASS_NAMES.with(|map| {
            map.borrow().iter().find(|(_, name)| *name == "Error").map(|(&id, _)| id)
        });
        let needs = HEAP.with(|heap| match heap.borrow().object(reference) {
            Some(fields) if (1000..=1012).contains(&fields.class_id) && dartforge_is_subclass(fields.class_id, 1007) != 0 => {
                let st_idx = match fields.class_id {
                    1003 => 5,
                    1004 => 7,
                    _ => 1,
                };
                // Without the stack trace field, nothing (an out-of-line body only
                // appears on a JIT reload: production code reads the body once per
                // object, `llvm/mod.rs`, `ajudante_do_corpo`).
                fields.get(st_idx).is_some_and(|f| f.0 == 0).then_some(st_idx)
            }
            Some(fields) if sdk_error.is_some_and(|cid| dartforge_is_subclass(fields.class_id, cid) != 0) => {
                fields.first().is_some_and(|(value_ref, _)| value_ref == 0).then_some(0)
            }
            _ => None,
        });
        if let Some(st_idx) = needs {
            // The stack trace allocates: the error stays rooted until it becomes
            // the pending exception (the thrower did not always root it).
            let st = with_roots(&[reference], || dartforge_stack_trace_get());
            HEAP.with(|heap| {
                let mut heap = heap.borrow_mut();
                heap.ensure_fields(reference, st_idx + 1);
                heap.set_field(reference, st_idx, st, true);
            });
        }
    }
    if debug() {
        let cid = if exception_ref(value).is_some() { dartforge_value_class(reference) } else { -100 };
        let name = CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned()).unwrap_or_default();
        eprintln!("[debug] throw: class {cid} {name}");
        show_trace();
    }
    // G6: the pending exception is a root until it is consumed.
    HEAP.with(|h| h.borrow_mut().set_runtime_root(0, reference));
    set_exception(Some(value));
}

/// Tells whether there is a pending exception on the current thread.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_pending() -> u8 {
    CONTEXT.with(|c| c.pending.get())
}

/// Disarms and clears the pending exception slot.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_clear() {
    if unwinding_now() {
        // Unwinding does not leave the pending state (`finally`, `return` and jumps
        // also clear): the pending exception reaches the event loop.
        return;
    }
    // Without an exception or a stored stack trace (the case of every `return`):
    // nothing to clear, and the heap is not touched.
    if CONTEXT.with(|c| c.pending.get()) == 0 && CURRENT_STACK_TRACE.with(|slot| slot.borrow().is_none()) {
        return;
    }
    take_exception();
    CURRENT_STACK_TRACE.with(|slot| {
        slot.borrow_mut().take();
    });
    forget_throw_returns();
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.set_runtime_root(0, 0);
        h.set_runtime_root(1, 0);
    });
}

fn allocate_format_exception(message: &str) -> i64 {
    let msg = error_text(message);
    if helper("_dartforgeErroDeFormato").is_some() {
        return with_roots(&[msg], || dartforge_format_exception_new(msg, 0, -1));
    }
    with_roots(&[msg], || {
        HEAP.with(|h| {
            // FormatException
            h.borrow_mut().new_object(1001, &[(msg, true), (0, true), (-1, false)])
        })
    })
}

// Core error constructors

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_new(msg_bits: i64, is_ref: u8) -> i64 {
    HEAP.with(|h| {
        let fields: &[crate::heap::Field] = if msg_bits == 0 && is_ref == 0 { &[] } else { &[(msg_bits, is_ref != 0)] };
        h.borrow_mut().new_object(1000, fields)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_format_exception_new(msg_handle: i64, src_handle: i64, offset: i64) -> i64 {
    if let Some(f) = helper("_dartforgeErroDeFormato") {
        // SAFETY: registered by `dart:core` with the signature
        // `(String, Object?, int) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(dart_target, a0, a1, a2) } };
        return with_roots(&[msg_handle, src_handle], || g(msg_handle, src_handle, offset));
    }
    HEAP.with(|h| {
        h.borrow_mut().new_object(1001, &[(msg_handle, true), (src_handle, true), (offset, false)])
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_state_error_new(msg_handle: i64) -> i64 {
    if let Some(f) = helper("_dartforgeErroDeEstado") {
        // SAFETY: registered by `dart:core` with the signature `(String) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        return with_roots(&[msg_handle], || g(msg_handle));
    }
    allocate_error_with_trace(1002, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_argument_error_new(msg_handle: i64, name_handle: i64) -> i64 {
    if let Some(f) = helper("_dartforgeErroDeArgumento") {
        // SAFETY: registered by `dart:core` with the signature
        // `(String?, String?) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64, a1: i64| -> i64 { dart_r2(dart_target, a0, a1) } };
        return with_roots(&[msg_handle, name_handle], || g(msg_handle, name_handle));
    }
    allocate_error_with_trace(1003, vec![(msg_handle, true), (name_handle, true), (0, false), (0, false), (0, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_argument_error_value(val_bits: i64, val_is_ref: u8, name_handle: i64, msg_handle: i64) -> i64 {
    allocate_error_with_trace(1003, vec![(msg_handle, true), (name_handle, true), (val_bits, val_is_ref != 0), (1, false), (0, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_argument_error_not_null(name_handle: i64) -> i64 {
    allocate_error_with_message(
        1003,
        "Must not be null",
        Vec::new(),
        vec![(name_handle, true), (0, false), (0, false), (0, false)],
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_range_error_new(msg_handle: i64) -> i64 {
    allocate_error_with_trace(1004, vec![(msg_handle, true), (0, false), (0, false), (0, false), (0, false), (0, false), (0, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_range_error_range(val: i64, min: i64, max: i64, name_handle: i64, msg_handle: i64) -> i64 {
    if let Some(f) = helper("_dartforgeErroDeFaixa") {
        // SAFETY: registered by `dart:core`: `(int, int, int, String?) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64, a1: i64, a2: i64, a3: i64| -> i64 { dart_r4(dart_target, a0, a1, a2, a3) } };
        let _ = msg_handle;
        return with_roots(&[name_handle], || g(val, min, max, name_handle));
    }
    allocate_error_with_trace(1004, vec![(msg_handle, true), (name_handle, true), (val, false), (min, false), (max, false), (1, false), (1, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_range_error_index(index: i64, indexable_or_len: i64, name_handle: i64, msg_handle: i64) -> i64 {
    // The length is read and the borrow released BEFORE asking for the stack trace
    // (G6: the previous version called another extern with `borrow_mut` open).
    // `indexable_or_len` is a list, a string or the length itself (a raw `int`,
    // which is only read as an object if it is a live block).
    let len = HEAP.with(|h| {
        let h = h.borrow();
        let x = indexable_or_len;
        if !crate::layout::is_object(x) || !h.is_live_object(x) {
            x
        } else if h.is_list(x) {
            h.list_len(x) as i64
        } else {
            h.text(x).map_or(x, |t| t.len() as i64)
        }
    });
    allocate_error_with_trace(
        1004,
        vec![(msg_handle, true), (name_handle, true), (index, false), (0, false), (len, false), (2, false), (1, false)],
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unsupported_error_new(msg_handle: i64) -> i64 {
    if let Some(f) = helper("_dartforgeErroNaoSuportado") {
        // SAFETY: registered by `dart:core` with the signature `(String?) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        return with_roots(&[msg_handle], || g(msg_handle));
    }
    allocate_error_with_trace(1005, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unimplemented_error_new(msg_handle: i64) -> i64 {
    allocate_error_with_trace(1008, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_assertion_error_new(msg_bits: i64, is_ref: u8) -> i64 {
    if let Some(f) = helper("_dartforgeErroDeAssercao") {
        // SAFETY: registered by `dart:core` with the signature `(Object?) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        let m = if is_ref != 0 { msg_bits } else { HEAP.with(|h| h.borrow_mut().to_ref(crate::heap::Value::Int(msg_bits))) };
        return with_roots(&[m], || g(m));
    }
    allocate_error_with_trace(1009, vec![(msg_bits, is_ref != 0)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_type_error_new() -> i64 {
    // SDK from source: the source's `_TypeError` (`identical_patch.dart`).
    if let Some(e) = source_error_with_text("_dartforgeErroDeTipo", "TypeError") {
        return e;
    }
    allocate_error_with_trace(1011, vec![(0, false)])
}

/// A `late` read/write error: 0/1 = field, 2/3 = local, 4/5 = write during the
/// initializer. The SDK source builds `LateError` to preserve the `is Error`
/// identity and the VM's `toString`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_error_new(name: i64, code: i64) -> i64 {
    if let Some(f) = helper("_dartforgeErroLate") {
        // SAFETY: helper registered by dart:_internal as (String, int) -> Object.
        let g = { let dart_target: usize = f; move |a0: i64, a1: i64| -> i64 { dart_r2(dart_target, a0, a1) } };
        return with_roots(&[name], || g(name, code));
    }
    let n = HEAP.with(|h| h.borrow().text(name).map(|t| t.to_dart_string()).unwrap_or_default());
    let (location, message) = match code {
        0 => ("Field", "has not been initialized"),
        1 => ("Field", "has already been initialized"),
        2 => ("Local", "has not been initialized"),
        3 => ("Local", "has already been initialized"),
        4 => ("Field", "has been assigned during initialization"),
        _ => ("Local", "has been assigned during initialization"),
    };
    // In the mode without the SDK from source, at least throw a catchable Error.
    // We do not reserve a synthetic CID: 1013 may belong to a real class.
    let text = format!("LateInitializationError: {location} '{n}' {message}.");
    let msg = error_text(&text);
    dartforge_state_error_new(msg)
}

/// Reentry of a global initializer: the VM would recurse in the getter until it
/// threw `StackOverflowError`. We build the same error without consuming the stack.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_stack_overflow_error_new() -> i64 {
    if let Some(f) = helper("_dartforgeErroPilha") {
        // SAFETY: helper registered by dart:_internal as () -> Object.
        let g = { let dart_target: usize = f; move || -> i64 { dart_r0(dart_target) } };
        return g();
    }
    let msg = error_text("Stack Overflow");
    dartforge_state_error_new(msg)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_no_such_method_error_new(name: i64) -> i64 {
    if debug() {
        let t = HEAP.with(|h| h.borrow().text(name).map(|t| t.to_dart_string()));
        eprintln!("[debug] NoSuchMethodError: {t:?}");
    }
    // In the SDK from source the error is always the SDK's `NoSuchMethodError` (the
    // synthetic class 1012 is not a subtype of anything in the compiled library's
    // RTI: not even a `catch (e)` would catch it).
    if let Some(f) = helper("_dartforgeErroDeChamada") {
        // The SDK from source provides the concrete NoSuchMethodError instance.
        // The name is rooted while the Dart constructors allocate.
        let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        return with_roots(&[name], || g(name));
    }
    allocate_error_with_trace(1012, vec![(name, true)])
}

/// The signature lookup is used only in the VM's detailed formatting. For an
/// invocation built by `Invocation.method`, the SDK's `toString` follows
/// `_toStringPlain` and does not consult this native; null is the "no signature
/// found" contract in the other cases.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_NoSuchMethodError_existingMethodSignature(
    _receiver: i64,
    _method_name: i64,
    _invocation_type: i64,
) -> i64 {
    0
}

// Core error getters

/// A field of a runtime error as a reference (R5): a stored scalar (the message of
/// `AssertionError` may be an `int`) comes out boxed; an absent one, null.
fn field_as_ref(handle: i64, ix: usize) -> i64 {
    let field = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Some(fields) = heap.object(handle) else { return None; };
        fields.get(ix)
    });
    match field {
        Some((bits, true)) => bits,
        Some((0, false)) | None => 0,
        Some((bits, false)) => HEAP.with(|h| h.borrow_mut().to_ref(crate::heap::Value::Int(bits))),
    }
}


/// SDK from source: the error built by the registered Dart function `name(text)`
/// (`identical_patch.dart`), or `None` without it.
fn source_error_with_text(name: &str, text: &str) -> Option<i64> {
    let f = helper(name)?;
    // SAFETY: registered by `dart:core` with the signature `(String) -> Object`.
    let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
    let t = error_text(text);
    Some(with_roots(&[t], || g(t)))
}

// The error constructors under ARC: the new object with a code owner. The `Ref`
// arguments are borrowed; the constructor may run Dart (inside a native scope)
// and leave a pending exception, in which case the result is null without an
// owner.

/// `Exception(msg)` under ARC, with a code owner ([`dartforge_exception_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_exception_new_owned_v1(msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_exception_new(msg, 1))
}

/// `AssertionError(msg)` under ARC, with a code owner ([`dartforge_assertion_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_assertion_error_new_owned_v1(msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_assertion_error_new(msg, 1))
}

/// `StateError(msg)` under ARC, with a code owner ([`dartforge_state_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_state_error_new_owned_v1(msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_state_error_new(msg))
}

/// `UnsupportedError(msg)` under ARC, with a code owner ([`dartforge_unsupported_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_unsupported_error_new_owned_v1(msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_unsupported_error_new(msg))
}

/// `UnimplementedError(msg)` under ARC, with a code owner ([`dartforge_unimplemented_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_unimplemented_error_new_owned_v1(msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_unimplemented_error_new(msg))
}

/// `ArgumentError(msg, name)` under ARC, with a code owner ([`dartforge_argument_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_argument_error_new_owned_v1(msg: i64, name: i64) -> i64 {
    owner_in_native_scope(|| dartforge_argument_error_new(msg, name))
}

/// `ArgumentError.value(value_ref, name, msg)` under ARC, with a code owner ([`dartforge_argument_error_value`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_argument_error_value_owned_v1(value_ref: i64, name: i64, msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_argument_error_value(value_ref, 1, name, msg))
}

/// `ArgumentError.notNull(name)` under ARC, with a code owner ([`dartforge_argument_error_not_null`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_argument_error_not_null_owned_v1(name: i64) -> i64 {
    owner_in_native_scope(|| dartforge_argument_error_not_null(name))
}

/// `RangeError.range(...)` under ARC, with a code owner ([`dartforge_range_error_range`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_range_error_range_owned_v1(value_ref: i64, min: i64, max: i64, name: i64, msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_range_error_range(value_ref, min, max, name, msg))
}

/// The runtime's `TypeError` under ARC, with a code owner ([`dartforge_type_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_type_error_new_owned_v1() -> i64 {
    owner_in_native_scope(|| dartforge_type_error_new())
}

/// The error of `!` on null under ARC, with a code owner ([`dartforge_null_check_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_null_check_error_new_owned_v1() -> i64 {
    owner_in_native_scope(|| dartforge_null_check_error_new())
}

/// The `LateError` of field/local `name` under ARC, with a code owner ([`dartforge_late_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_late_error_new_owned_v1(name: i64, code: i64) -> i64 {
    owner_in_native_scope(|| dartforge_late_error_new(name, code))
}

/// The reentry `StackOverflowError` under ARC, with a code owner ([`dartforge_stack_overflow_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_stack_overflow_error_new_owned_v1() -> i64 {
    owner_in_native_scope(|| dartforge_stack_overflow_error_new())
}

/// The `NoSuchMethodError` of `name` under ARC, with a code owner ([`dartforge_no_such_method_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_no_such_method_error_new_owned_v1(name: i64) -> i64 {
    owner_in_native_scope(|| dartforge_no_such_method_error_new(name))
}
