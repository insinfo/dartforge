// The runtime side of the JavaScript interop of WebAssembly modules
// (`dart:js_interop`, docs/PLANO-WASM-MVP.md, "Desenho do interop JS"),
// compiled only into the wasm32 runtime (build.rs, `WASM_ONLY`).
//
// A JavaScript value lives in the adapter's table (runtime/wasm/
// dartforge_host_v1.mjs) and Dart sees its handle, a positive `i32`; handle 0 is
// JavaScript `null`/`undefined`, which Dart sees as `null`, as in dart2wasm.
// `JSValue` (`dart:_js_helper`) owns one handle and attaches a native finalizer
// that returns it to the table when the object dies. Texts cross by copying
// their code units through linear memory.

#[link(wasm_import_module = "dartforge_js")]
unsafe extern "C" {
    #[link_name = "release"]
    fn js_release(handle: i32);
    #[link_name = "string_from_latin1"]
    fn js_string_from_latin1(offset: i32, length: i32) -> i32;
    #[link_name = "string_from_utf16"]
    fn js_string_from_utf16(offset: i32, length: i32) -> i32;
    #[link_name = "string_length"]
    fn js_string_length(handle: i32) -> i32;
    #[link_name = "string_read"]
    fn js_string_read(handle: i32, offset: i32, two_bytes: i32);
}

/// The native finalizer of a `JSValue`: returns its handle to the adapter's
/// table. It runs during the collection and touches only the JavaScript side.
extern "C" fn release_js_handle(handle: usize) {
    // SAFETY: an import without memory arguments; the handle came from the table.
    unsafe { js_release(handle as i32) }
}

/// `DartForge_js_attach(value, handle)`: `value` (a new `JSValue`) owns
/// `handle`; the collection that finds it dead releases the handle.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_js_attach(value: i64, handle: i64) {
    if handle == 0 {
        return;
    }
    HEAP.with(|h| {
        h.borrow_mut().add_attachment(crate::heap::FinalizerAttachment {
            owner: 0,
            value,
            detach: 0,
            action: crate::heap::FinalizerAction::Native(release_js_handle as usize, handle as usize, 0),
        })
    });
}

/// A new JavaScript string with the units of the Dart string `s` (`null`: handle
/// 0). The code of a snippet argument of type `String` calls it right before the
/// import, and the snippet's wrapper consumes the handle.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_js_string_to_js(s: i64) -> i64 {
    if s == 0 {
        return 0;
    }
    HEAP.with(|h| {
        let h = h.borrow();
        let text = h.text(s).expect("compiler bug: a String argument that is not a string");
        // SAFETY: the units are a live range of linear memory, read during the
        // call; the heap borrow keeps the collector away.
        let handle = unsafe {
            match text {
                crate::text::TextRef::OneByte(b) => js_string_from_latin1(b.as_ptr() as i32, b.len() as i32),
                crate::text::TextRef::TwoByte(u) => js_string_from_utf16(u.as_ptr() as i32, u.len() as i32),
            }
        };
        i64::from(handle)
    })
}

/// The Dart string with the units of the JavaScript string `handle`, which the
/// call consumes (handle 0: `null`). The adapter answers the length with the
/// sign of the form: `n` units that all fit in one byte, or `-n - 1`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_js_string_to_dart(handle: i64) -> i64 {
    if handle == 0 {
        return 0;
    }
    let handle = handle as i32;
    // SAFETY: an import without memory arguments.
    let answer = unsafe { js_string_length(handle) };
    let (len, two) = if answer < 0 { ((-(answer + 1)) as usize, true) } else { (answer as usize, false) };
    let s = HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let (s, units) = h.new_string_units(len, two);
        // SAFETY: `units` has room for `len` units of the chosen form and the
        // string is not published yet; the adapter writes exactly them and does
        // not reenter the module.
        unsafe { js_string_read(handle, units as i32, i32::from(two)) };
        s
    });
    release_js_handle(handle as usize);
    s
}

/// The JavaScript exception a snippet threw: the adapter's wrapper calls it
/// before returning to the module, and the call that ran the snippet finds the
/// exception pending. The Dart side sees dart2wasm's `_JavaScriptError`
/// (`dart:_js_helper`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_js_throw() {
    let Some(f) = helper("_dartforgeJavaScriptError") else {
        throw_internal_error("JavaScript exception without dart:_js_helper");
        return;
    };
    let error = dart_r0(f);
    if dartforge_exception_pending() != 0 {
        return;
    }
    with_roots(&[error], || dartforge_exception_throw(error, 3));
}

// Dart objects that JavaScript holds (`jsObjectFromDartObject`, `toJSBox`,
// `ExternalDartReference`, the callbacks of `Function.toJS`): an id per object,
// a global root while JavaScript keeps it, and the same id for the same object.
// The adapter makes one `DartRef` per id and a `FinalizationRegistry` releases
// the id (`dartforge_js_dart_release`) when JavaScript drops it.

#[derive(Default)]
struct DartRefs {
    objects: Vec<Option<i64>>,
    free: Vec<usize>,
    ids: crate::hash::HashMap<i64, usize>,
}

/// The exceptions on their way through JavaScript (a Dart exception a callback
/// threw): the value, the stack trace and the throw's return addresses, until
/// a snippet's wrapper rethrows them in Dart or JavaScript drops them.
struct Stashed {
    value: crate::heap::Value,
    trace: Option<i64>,
    returns: Vec<(u64, u64)>,
}

thread_local! {
    static DART_REFS: RefCell<DartRefs> = RefCell::new(DartRefs::default());
    static STASHED: RefCell<Vec<Option<Stashed>>> = const { RefCell::new(Vec::new()) };
}

/// The global root key of slot `slot` of table `table` (1: Dart objects, 2: the
/// stashed exceptions, 3: their stack traces): far below any address.
fn js_root_key(table: i64, slot: usize) -> i64 {
    i64::MIN + (table << 40) + slot as i64
}

/// Whether JavaScript still holds Dart objects and nothing is pending: the
/// program waits for the host (`wasm_entry.rs`).
fn js_host_waiting() -> bool {
    dartforge_exception_pending() == 0 && DART_REFS.with(|r| r.borrow().ids.len()) > 0
}

/// `DartForge_js_dart_ref(object)`: the id JavaScript refers to `object` by.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_js_dart_ref(object: i64) -> i64 {
    let id = DART_REFS.with(|r| {
        let mut r = r.borrow_mut();
        if let Some(&id) = r.ids.get(&object) {
            return id;
        }
        let id = match r.free.pop() {
            Some(id) => id,
            None => {
                r.objects.push(None);
                r.objects.len() - 1
            }
        };
        r.objects[id] = Some(object);
        r.ids.insert(object, id);
        id
    });
    HEAP.with(|h| h.borrow_mut().set_global_root(js_root_key(1, id), object));
    id as i64
}

/// `DartForge_js_dart_object(id)`: the object of an id JavaScript holds.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_js_dart_object(id: i64) -> i64 {
    dart_ref_object(id as i32)
}

fn dart_ref_object(id: i32) -> i64 {
    DART_REFS
        .with(|r| r.borrow().objects.get(id as usize).copied().flatten())
        .unwrap_or_else(|| panic!("JavaScript refers to a released Dart object ({id})"))
}

/// JavaScript dropped the `DartRef` of `id` (the adapter's `FinalizationRegistry`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_js_dart_release(id: i32) {
    let released = DART_REFS.with(|r| {
        let mut r = r.borrow_mut();
        let object = r.objects.get_mut(id as usize)?.take()?;
        r.ids.remove(&object);
        r.free.push(id as usize);
        Some(())
    });
    if released.is_some() {
        HEAP.with(|h| h.borrow_mut().release_global_root(js_root_key(1, id as usize)));
    }
}

/// Takes the pending exception out of Dart, rooted, and returns its id.
fn stash_pending_exception() -> i32 {
    let value = take_exception().expect("a pending exception to stash");
    let trace = CURRENT_STACK_TRACE.with(|slot| slot.borrow_mut().take());
    let returns = THROW_RETURNS.with(|r| std::mem::take(&mut *r.borrow_mut()));
    let id = STASHED.with(|s| {
        let mut s = s.borrow_mut();
        let id = s.iter().position(Option::is_none).unwrap_or_else(|| {
            s.push(None);
            s.len() - 1
        });
        s[id] = Some(Stashed { value, trace, returns });
        id
    });
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.set_global_root(js_root_key(2, id), exception_ref(value).unwrap_or(0));
        h.set_global_root(js_root_key(3, id), trace.unwrap_or(0));
        h.set_runtime_root(0, 0);
        h.set_runtime_root(1, 0);
    });
    id as i32
}

fn unstash(id: i32) -> Option<Stashed> {
    let stashed = STASHED.with(|s| s.borrow_mut().get_mut(id as usize)?.take())?;
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.release_global_root(js_root_key(2, id as usize));
        h.release_global_root(js_root_key(3, id as usize));
    });
    Some(stashed)
}

/// A snippet's wrapper caught a Dart exception on its way through JavaScript:
/// it becomes pending again, as it was thrown.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_js_rethrow(id: i32) {
    let Some(s) = unstash(id) else {
        return;
    };
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.set_runtime_root(0, exception_ref(s.value).unwrap_or(0));
        h.set_runtime_root(1, s.trace.unwrap_or(0));
    });
    CURRENT_STACK_TRACE.with(|slot| *slot.borrow_mut() = s.trace);
    THROW_RETURNS.with(|r| *r.borrow_mut() = s.returns);
    set_exception(Some(s.value));
}

/// JavaScript dropped a Dart exception without rethrowing it in Dart.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_js_discard(id: i32) {
    unstash(id);
}

/// A Dart exception reached the host's event loop: the program ends as with an
/// unhandled exception (`Unhandled exception:`, the message, status 255).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_js_report(id: i32) -> i32 {
    dartforge_js_rethrow(id);
    finish_program()
}

/// JavaScript calls the Dart callback `callback` (a `DartRef` id) with `argc`
/// arguments, which the callback reads from the adapter, through `trampoline`
/// (the address of `dart:_js_helper`'s `_jsCall`, which `Function.toJS` gave
/// the wrapper). Returns the handle of the result, or `-1 - id` of the stashed
/// exception the callback threw.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_js_call(trampoline: i32, callback: i32, argc: i32) -> i32 {
    let target = dart_ref_object(callback);
    let result = with_roots(&[target], || dart_r2(trampoline as u32 as usize, target, i64::from(argc)));
    if dartforge_exception_pending() != 0 {
        return -1 - stash_pending_exception();
    }
    result as i32
}

/// After a callback the host's event loop made: runs the program's own events
/// that became due (microtasks, timers). Returns the microseconds until the
/// next timer, `-1` while only the host can call back, or `-2` when the program
/// ended (an unhandled exception ends the module through `exit`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_js_drain() -> f64 {
    resume_event_loop();
    match loop_status() {
        PENDING => event_loop_delay_micros(),
        WAITING => -1.0,
        _ => -2.0,
    }
}

/// `DartForge_js_typed_data_address(data)`: the address of the first byte of a
/// typed list of the runtime (internal, external or view), for the snippet that
/// copies it into a JavaScript typed array; -1 for any other object.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_js_typed_data_address(data: i64) -> i64 {
    HEAP.with(|h| h.borrow().typed(data)).map_or(-1, |t| t.data as usize as i64)
}
