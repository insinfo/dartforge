// Native runtime: Dart's native port API (`dart_native_api.h`) and the data of
// `dart_api_dl.h` — what `NativeApi.postCObject`, `NativeApi.newNativePort`,
// `NativeApi.closeNativePort` and `NativeApi.initializeApiDLData` hand to C code.
//
// * `Dart_PostCObject(port, object)`: reads the `Dart_CObject` tree (on any
//   thread, without the Dart heap), builds the portable message (`Portable`) and
//   posts it to the port, like `SendPort.send`. External data
//   (`kExternalTypedData`) is copied and the C finalizer is called right away —
//   the VM calls it when the object dies; here the message no longer depends on
//   its memory.
// * `Dart_NewNativePort(name, handler, concurrent)`: a port served by its own
//   thread, which converts each message back to `Dart_CObject` and calls the
//   handler (the VM uses the thread pool; the messages of one port arrive in
//   order, one at a time).
// * `initializeApiDLData`: the `DartApi` table of version 2.5 of the DL API with
//   the native API functions; the names that depend on `Dart_Handle` (the full
//   embedder API) are not listed, and `Dart_InitializeApiDL` leaves them null —
//   as it would for a missing symbol.
//
// Layout of `Dart_CObject` (C, 64 bits): `type` (int32) at 0 and the union at 8,
// with 40 bytes; the fields of each variant follow the header.

use std::ffi::{c_char, CStr, CString};

const KNULL: i32 = 0;
const KBOOL: i32 = 1;
const KINT32: i32 = 2;
const KINT64: i32 = 3;
const KDOUBLE: i32 = 4;
const KSTRING: i32 = 5;
const KARRAY: i32 = 6;
const KTYPED_DATA: i32 = 7;
const KEXTERNAL_TYPED_DATA: i32 = 8;
const KSEND_PORT: i32 = 9;
const KCAPABILITY: i32 = 10;
const KNATIVE_POINTER: i32 = 11;
const KUNMODIFIABLE_EXTERNAL_TYPED_DATA: i32 = 13;

/// Size of a `Dart_CObject` (the `type` and the 40-byte union).
const COBJECT_SIZE: usize = 48;

/// `Dart_TypedData_Type` → the runtime's `TIPO_*`.
fn typed_data_type(t: i32) -> Option<u8> {
    Some(match t {
        0 => TYPE_BYTE_DATA,
        1 => TYPE_INT8,
        2 => TYPE_UINT8,
        3 => TYPE_UINT8_CLAMPED,
        4 => TYPE_INT16,
        5 => TYPE_UINT16,
        6 => TYPE_INT32,
        7 => TYPE_UINT32,
        8 => TYPE_INT64,
        9 => TYPE_UINT64,
        10 => TYPE_FLOAT32,
        11 => TYPE_FLOAT64,
        12 => TYPE_INT32X4,
        13 => TYPE_FLOAT32X4,
        14 => TYPE_FLOAT64X2,
        _ => return None,
    })
}

/// The inverse of [`typed_data_type`].
fn typed_data_of_type(t: u8) -> i32 {
    match t {
        TYPE_BYTE_DATA => 0,
        TYPE_INT8 => 1,
        TYPE_UINT8 => 2,
        TYPE_UINT8_CLAMPED => 3,
        TYPE_INT16 => 4,
        TYPE_UINT16 => 5,
        TYPE_INT32 => 6,
        TYPE_UINT32 => 7,
        TYPE_INT64 => 8,
        TYPE_UINT64 => 9,
        TYPE_FLOAT32 => 10,
        TYPE_FLOAT64 => 11,
        TYPE_INT32X4 => 12,
        TYPE_FLOAT32X4 => 13,
        TYPE_FLOAT64X2 => 14,
        _ => 2,
    }
}

type HandleFinalizer = extern "C" fn(usize, usize);

/// Reads the `Dart_CObject` tree at `p`. `None`: unsupported type (the VM rejects
/// the `Dart_PostCObject`). The finalizers of the external data read go to
/// `external`, called only if the message is posted.
///
/// # Safety
/// `p` points to a valid `Dart_CObject` of the caller.
unsafe fn read_cobject(p: *const u8, external: &mut Vec<(usize, usize)>, depth: usize) -> Option<Portable> {
    if p.is_null() || depth > 10_000 {
        return None;
    }
    // SAFETY: guaranteed by the caller; every read is within the object's 48
    // bytes, at the header's positions.
    unsafe {
        let ty = *(p as *const i32);
        let v = p.add(8);
        Some(match ty {
            KNULL => Portable::Null,
            KBOOL => Portable::Bool(*v != 0),
            KINT32 => Portable::Int(i64::from(*(v as *const i32))),
            KINT64 => Portable::Int(*(v as *const i64)),
            KDOUBLE => Portable::Double(*(v as *const f64)),
            KSTRING => {
                let s = *(v as *const *const c_char);
                if s.is_null() {
                    return None;
                }
                Portable::Str(CStr::from_ptr(s).to_str().ok()?.to_owned())
            }
            KARRAY => {
                let n = *(v as *const isize);
                let value_list = *(v.add(8) as *const *const *const u8);
                if n < 0 || (n > 0 && value_list.is_null()) {
                    return None;
                }
                let mut items = Vec::with_capacity(n as usize);
                for i in 0..n as usize {
                    items.push(read_cobject(*value_list.add(i), external, depth + 1)?);
                }
                Portable::List(items)
            }
            KTYPED_DATA | KEXTERNAL_TYPED_DATA | KUNMODIFIABLE_EXTERNAL_TYPED_DATA => {
                let t = typed_data_type(*(v as *const i32))?;
                let n = *(v.add(8) as *const isize);
                let data = *(v.add(16) as *const *const u8);
                if n < 0 || (n > 0 && data.is_null()) {
                    return None;
                }
                let bytes = n as usize * element_size(t);
                let copy = if bytes == 0 { Vec::new() } else { std::slice::from_raw_parts(data, bytes).to_vec() };
                if ty != KTYPED_DATA {
                    let pair = *(v.add(24) as *const usize);
                    let finalizer = *(v.add(32) as *const usize);
                    if finalizer != 0 {
                        external.push((finalizer, pair));
                    }
                }
                Portable::Typed(t, copy)
            }
            KSEND_PORT => Portable::FromRuntime { cid: crate::layout::cid::SEND_PORT, id: *(v as *const i64) },
            KCAPABILITY => Portable::FromRuntime { cid: crate::layout::cid::CAPABILITY, id: *(v as *const i64) },
            // The address arrives as an `int`; the finalizer stays with C (the VM
            // only calls it if the message is not delivered).
            KNATIVE_POINTER => Portable::Int(*(v as *const i64)),
            _ => return None,
        })
    }
}

/// Whether `port` exists (open in some isolate or service).
fn port_exists(port: i64) -> bool {
    rec().lock().unwrap_or_else(|e| e.into_inner()).contains_key(&port)
}

/// `Dart_PostCObject(port_id, message)`.
///
/// # Safety
/// `message` points to a valid `Dart_CObject`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_api_post_cobject(port: i64, message: *const u8) -> bool {
    let mut external = Vec::new();
    // SAFETY: guaranteed by the caller.
    let Some(value) = (unsafe { read_cobject(message, &mut external, 0) }) else {
        return false;
    };
    if !port_exists(port) {
        return false;
    }
    post(port, value.to_graph());
    for (f, pair) in external {
        // SAFETY: the caller's `Dart_HandleFinalizer`, `(isolate_data, peer)`.
        let f: HandleFinalizer = unsafe { std::mem::transmute(f) };
        f(0, pair);
    }
    true
}

/// `Dart_PostInteger(port_id, message)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_api_post_integer(port: i64, value: i64) -> bool {
    if !port_exists(port) {
        return false;
    }
    post(port, Portable::Int(value).to_graph());
    true
}

/// The C objects built for the handler of a native port; alive until it returns.
#[derive(Default)]
struct CObjectArena {
    objects: Vec<Box<[u64; COBJECT_SIZE / 8]>>,
    text: Vec<CString>,
    lists: Vec<Vec<*mut u8>>,
    bytes: Vec<Vec<u8>>,
}

impl CObjectArena {
    fn new(&mut self, ty: i32) -> *mut u8 {
        let mut o = Box::new([0u64; COBJECT_SIZE / 8]);
        o[0] = ty as u32 as u64;
        let p = o.as_mut_ptr() as *mut u8;
        self.objects.push(o);
        p
    }

    /// The `Dart_CObject` of `v`.
    fn build(&mut self, v: &Portable) -> *mut u8 {
        // SAFETY: every write stays within the 48 bytes of the freshly created
        // object, at the header's positions.
        unsafe {
            match v {
                Portable::Null | Portable::Object(_) => self.new(KNULL),
                Portable::Bool(b) => {
                    let p = self.new(KBOOL);
                    *p.add(8) = u8::from(*b);
                    p
                }
                Portable::Int(x) => {
                    let p = self.new(KINT64);
                    *(p.add(8) as *mut i64) = *x;
                    p
                }
                Portable::Double(x) => {
                    let p = self.new(KDOUBLE);
                    *(p.add(8) as *mut f64) = *x;
                    p
                }
                Portable::Str(s) => {
                    let p = self.new(KSTRING);
                    let c = CString::new(s.replace('\0', "")).unwrap_or_default();
                    *(p.add(8) as *mut *const c_char) = c.as_ptr();
                    self.text.push(c);
                    p
                }
                Portable::List(items) => {
                    let children: Vec<*mut u8> = items.iter().map(|x| self.build(x)).collect();
                    let p = self.new(KARRAY);
                    *(p.add(8) as *mut isize) = children.len() as isize;
                    *(p.add(16) as *mut *const *mut u8) = children.as_ptr();
                    self.lists.push(children);
                    p
                }
                Portable::Bytes(b) | Portable::Typed(_, b) => {
                    let t = if let Portable::Typed(t, _) = v { *t } else { TYPE_UINT8 };
                    let p = self.new(KTYPED_DATA);
                    *(p.add(8) as *mut i32) = typed_data_of_type(t);
                    *(p.add(16) as *mut isize) = (b.len() / element_size(t).max(1)) as isize;
                    self.bytes.push(b.clone());
                    let last_one = self.bytes.last().expect("just inserted");
                    *(p.add(24) as *mut *const u8) = last_one.as_ptr();
                    p
                }
                Portable::FromRuntime { cid, id } => {
                    let p = self.new(if *cid == crate::layout::cid::CAPABILITY { KCAPABILITY } else { KSEND_PORT });
                    *(p.add(8) as *mut i64) = *id;
                    p
                }
            }
        }
    }
}

type NativePortHandler = extern "C" fn(i64, *mut u8);

/// The open native ports: id → queue of the thread that serves them.
fn native_ports() -> &'static std::sync::Mutex<crate::hash::HashMap<i64, std::sync::mpsc::Sender<(i64, Graph)>>> {
    static T: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<i64, std::sync::mpsc::Sender<(i64, Graph)>>>> =
        std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// `Dart_NewNativePort(name, handler, handle_concurrently)`: the port id
/// (0 = `ILLEGAL_PORT`).
///
/// # Safety
/// `name` is null or a C string; `handler` is `void(Dart_Port, Dart_CObject*)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_api_new_native_port(name: *const c_char, handler: usize, _concurrent: bool) -> i64 {
    if handler == 0 {
        return 0;
    }
    let name = if name.is_null() {
        "native port".to_owned()
    } else {
        // SAFETY: guaranteed by the caller.
        unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned()
    };
    // SAFETY: the signature of `Dart_NativeMessageHandler`.
    let handler: NativePortHandler = unsafe { std::mem::transmute(handler) };
    let (tx, rx) = std::sync::mpsc::channel::<(i64, Graph)>();
    let queue = std::sync::Mutex::new(tx.clone());
    let port = open_native_port(std::sync::Arc::new(move |port, graph| {
        let _ = queue.lock().unwrap_or_else(|e| e.into_inner()).send((port, graph));
    }));
    let created = std::thread::Builder::new().name(name).spawn(move || {
        for (port, graph) in rx {
            let mut arena = CObjectArena::default();
            let object = arena.build(&graph.to_portable());
            handler(port, object);
        }
    });
    if created.is_err() {
        close_native_port(port);
        return 0;
    }
    native_ports().lock().unwrap_or_else(|e| e.into_inner()).insert(port, tx);
    port
}

/// `Dart_CloseNativePort(native_port_id)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_api_close_native_port(port: i64) -> bool {
    let Some(tx) = native_ports().lock().unwrap_or_else(|e| e.into_inner()).remove(&port) else {
        return false;
    };
    close_native_port(port);
    // Without the registration and this end, the thread's queue closes and the
    // thread ends after the messages already delivered.
    drop(tx);
    true
}

/// The native API functions, by their name in `dart_native_api.h`.
fn native_api_functions() -> [(&'static str, usize); 4] {
    [
        ("Dart_PostCObject", dartforge_api_post_cobject as usize),
        ("Dart_PostInteger", dartforge_api_post_integer as usize),
        ("Dart_NewNativePort", dartforge_api_new_native_port as usize),
        ("Dart_CloseNativePort", dartforge_api_close_native_port as usize),
    ]
}

/// `DartNativeApiFunctionPointer(name)`: the address of the native API function.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartNativeApiFunctionPointer(name: i64) -> i64 {
    let name = HEAP.with(|h| h.borrow().text(name).map(|t| t.to_dart_string()).unwrap_or_default());
    match native_api_functions().iter().find(|(n, _)| *n == name) {
        Some((_, f)) => *f as i64,
        None => {
            throw_argument_error(&format!("Unknown native API function: {name}"));
            0
        }
    }
}

/// The version of the DL API (`DART_API_DL_MAJOR_VERSION`/`MINOR`) of the
/// reference VM.
const DL_API_VERSION: (i32, i32) = (2, 5);

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartApiDLMajorVersion() -> i64 {
    i64::from(DL_API_VERSION.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartApiDLMinorVersion() -> i64 {
    i64::from(DL_API_VERSION.1)
}

/// `DartApiEntry { const char* name; void (*function)(void); }`.
#[repr(C)]
struct ApiEntry {
    name: *const c_char,
    function: usize,
}

/// `DartApi { const int major; const int minor; const DartApiEntry* const functions; }`.
#[repr(C)]
struct ApiData {
    larger: i32,
    minor: i32,
    functions: *const ApiEntry,
}

/// The address of the data, built once and alive for the whole process.
fn api_data() -> usize {
    static DATA: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *DATA.get_or_init(|| {
        let mut entries: Vec<ApiEntry> = native_api_functions()
            .iter()
            .map(|(n, f)| ApiEntry { name: CString::new(*n).expect("C name").into_raw(), function: *f })
            .collect();
        entries.push(ApiEntry { name: std::ptr::null(), function: 0 });
        let functions = Box::leak(entries.into_boxed_slice()).as_ptr();
        let data = Box::leak(Box::new(ApiData { larger: DL_API_VERSION.0, minor: DL_API_VERSION.1, functions }));
        data as *const ApiData as usize
    })
}

/// `DartApiDLInitializeData()`: the pointer that C passes to
/// `Dart_InitializeApiDL`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartApiDLInitializeData() -> i64 {
    api_data() as i64
}
