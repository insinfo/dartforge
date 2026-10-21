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
