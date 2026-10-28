// Dart values and errors that natives of every target build: the shared helpers
// of the fragments (formerly in the `dart:io` ones, which WebAssembly leaves out).

/// Throws `ArgumentError(message)` (the VM's API errors).
fn throw_argument_error(message: &str) {
    let msg = allocate_str(message);
    let e = with_roots(&[msg], || dartforge_argument_error_new(msg, 0));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

/// Throws `StateError(message)` (the `_InternalError` of the VM's natives).
fn throw_internal_error(message: &str) {
    let e = allocate_state_error(message);
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

/// A fixed-length `_List` (cid 8, general) with the values (already in the `Ref`
/// position), rooted during the allocation.
fn dart_fixed_list(value_list: &[i64]) -> i64 {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let frame = h.push_owning_frame(value_list.len().max(1));
        for (i, &v) in value_list.iter().enumerate() {
            h.set_root(frame, i, v);
        }
        let l = h.new_list(crate::layout::cid::LIST, value_list.len(), crate::lists::Element::General);
        // Word 0 of the body is the length; the elements, the following ones.
        if !value_list.is_empty() {
            h.write_refs(l, 1, value_list);
        }
        // The list's edges replace the temporary owning copies.
        h.pop_frame(frame);
        l
    })
}

/// The native field of a `NativeFieldWrapperClass1` (the `dart:nativewrappers`
/// overlay declares it as the first field of the layout).
fn native_field(obj: i64) -> i64 {
    HEAP.with(|h| match h.borrow().object(obj) {
        Some(o) => o.first().map_or(0, |f| f.0),
        None => panic!("compiler bug: native field of something that is not an object: {obj}"),
    })
}

/// The system path for the bytes (Unix: the bytes; Windows: the UTF-8 converted to
/// UTF-16, like the VM's `Utf8ToWideChar`; WebAssembly: the UTF-8 text).
#[cfg(unix)]
fn path_from_bytes(b: &[u8]) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStrExt;
    std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b))
}
#[cfg(any(windows, target_arch = "wasm32"))]
fn path_from_bytes(b: &[u8]) -> std::path::PathBuf {
    std::path::PathBuf::from(String::from_utf8_lossy(b).into_owned())
}

/// The `Platform.script` when the process is not the program itself (the JIT): the
/// bytes of the `.dart` path. `dart:io` sets it (fragment `io_platform`) and the
/// isolates read it; a target without `dart:io` keeps `None`.
fn program_script() -> &'static std::sync::Mutex<Option<Vec<u8>>> {
    static S: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);
    &S
}

/// A fixed-length `_List` with the strings; `typed`: `List<String>` (the VM's
/// `Dart_NewListOfTypeFilled`), otherwise `List<dynamic>`.
fn dart_string_list(text: &[String], typed: bool) -> i64 {
    let list = dart_fixed_list(&vec![0; text.len()]);
    with_roots(&[list], || {
        for (i, t) in text.iter().enumerate() {
            let s = allocate_str(t);
            HEAP.with(|h| h.borrow_mut().list_set(list, i, crate::heap::Value::Ref(s)));
        }
        if typed {
            let cid = HEAP.with(|h| h.borrow().class(list));
            if let Some(ty) = string_list_type(Some(i64::from(cid))) {
                HEAP.with(|h| h.borrow_mut().set_metadata(list, ty + 1));
            }
        }
    });
    list
}
