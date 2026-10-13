// Native runtime: per-class method tables and lookup by selector, for the SDK
// compiled from source (P5c, δ; `crates/emit_native/src/llvm/seletores.rs`).
//
// Each class registers its table at startup: pairs (selector hash, uniform entry),
// sorted by hash. A call by selector goes through the call site's cache (class id
// + 1, entry); on a miss, it searches the table of the receiver's dynamic class. A
// selector that no class has gives the `NoSuchMethodError` entry.

thread_local! {
    /// Method table by class id: (pointer to the pairs, how many).
    /// The per-class method tables: `n` pairs `[hash, input]` sorted by hash,
    /// **copied** from the module constant — the memory of a JIT generation may be
    /// freed after a reload (J02), and an `Arc` because messages between isolates
    /// carry the table along.
    static METHODS: RefCell<HashMap<i64, MethodTable>> = RefCell::new(HashMap::default());
    /// The classes with an already registered table, by id (the test of every
    /// allocation in `dartforge_object_new_t`, without the `METHODS` hash). It only
    /// grows, like `METHODS`.
    static REGISTERED: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
    /// The selector of the last failed lookup (the text of the `NoSuchMethodError`).
    static MISSING_SELECTOR: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The positions of the table of 32 runtime classes that the emitter writes
/// (`sdk_module::cids_do_runtime`, `@df.cids`). With the fixed cids
/// (docs/NATIVO-ESPACO-UNIFICADO.md §2.4) the table is only the ABI check of
/// [`dartforge_register_cids`]; the positions remain for whoever still asks for
/// the cid through them.
const CID_NULL: usize = 0;
const CID_SMI: usize = 1;
const CID_MINT: usize = 2;
const CID_DOUBLE: usize = 3;
const CID_BOOL: usize = 4;
const CID_ONE_BYTE_STRING: usize = 5;
const CID_TWO_BYTE_STRING: usize = 6;
const CID_GROWABLE_LIST: usize = 7;
const CID_LIST: usize = 8;
const CID_IMMUTABLE_LIST: usize = 9;
const CID_CLOSURE: usize = 10;
const CID_RECORD: usize = 11;
const CID_UINT8_LIST: usize = 12;
const CID_UINT8_VIEW: usize = 13;
const CID_INT64_LIST: usize = 14;
// 15–17: the SIMD values (`simd.rs`).
const CID_INT8_LIST: usize = 18;
const CID_UINT8_CLAMPED_LIST: usize = 19;
const CID_INT16_LIST: usize = 20;
const CID_UINT16_LIST: usize = 21;
const CID_INT32_LIST: usize = 22;
const CID_UINT32_LIST: usize = 23;
const CID_UINT64_LIST: usize = 24;
const CID_FLOAT32_LIST: usize = 25;
const CID_FLOAT64_LIST: usize = 26;
const CID_INT32X4_LIST: usize = 27;
const CID_FLOAT32X4_LIST: usize = 28;
const CID_FLOAT64X2_LIST: usize = 29;
const CID_SEND_PORT: usize = 30;
const CID_CAPABILITY: usize = 31;

/// The fixed cid of each position of the runtime classes table (the order of
/// `CID_*`; `layout::cid`).
const FIXED_CIDS_BY_POSITION: [i32; 32] = {
    use crate::layout::cid;
    [
        cid::NULL,
        cid::SMI,
        cid::MINT,
        cid::DOUBLE,
        cid::BOOL,
        cid::ONE_BYTE_STRING,
        cid::TWO_BYTE_STRING,
        cid::GROWABLE_LIST,
        cid::LIST,
        cid::IMMUTABLE_LIST,
        cid::CLOSURE,
        cid::RECORD,
        cid::FIRST_TYPED + 1,  // _Uint8List
        cid::FIRST_VIEW + 1,   // _Uint8ArrayView
        cid::FIRST_TYPED + 7,  // _Int64List
        cid::FLOAT32X4,
        cid::INT32X4,
        cid::FLOAT64X2,
        cid::FIRST_TYPED,      // _Int8List
        cid::FIRST_TYPED + 2,  // _Uint8ClampedList
        cid::FIRST_TYPED + 3,  // _Int16List
        cid::FIRST_TYPED + 4,  // _Uint16List
        cid::FIRST_TYPED + 5,  // _Int32List
        cid::FIRST_TYPED + 6,  // _Uint32List
        cid::FIRST_TYPED + 8,  // _Uint64List
        cid::FIRST_TYPED + 9,  // _Float32List
        cid::FIRST_TYPED + 10, // _Float64List
        cid::FIRST_TYPED + 12, // _Int32x4List
        cid::FIRST_TYPED + 11, // _Float32x4List
        cid::FIRST_TYPED + 13, // _Float64x2List
        cid::SEND_PORT,
        cid::CAPABILITY,
    ]
};

const _: () = {
    // The named positions match the table.
    assert!(FIXED_CIDS_BY_POSITION[CID_NULL] == crate::layout::cid::NULL);
    assert!(FIXED_CIDS_BY_POSITION[CID_SMI] == crate::layout::cid::SMI);
    assert!(FIXED_CIDS_BY_POSITION[CID_UINT8_LIST] == crate::layout::cid::typed(1));
    assert!(FIXED_CIDS_BY_POSITION[CID_UINT8_VIEW] == crate::layout::cid::view(1, false));
    assert!(FIXED_CIDS_BY_POSITION[CID_INT64_LIST] == crate::layout::cid::typed(7));
    assert!(FIXED_CIDS_BY_POSITION[CID_INT8_LIST] == crate::layout::cid::typed(0));
    assert!(FIXED_CIDS_BY_POSITION[CID_FLOAT64X2_LIST] == crate::layout::cid::typed(13));
    assert!(FIXED_CIDS_BY_POSITION[CID_SEND_PORT] == crate::layout::cid::SEND_PORT);
    assert!(FIXED_CIDS_BY_POSITION[CID_CAPABILITY] == crate::layout::cid::CAPABILITY);
    assert!(CID_MINT + CID_DOUBLE + CID_BOOL + CID_ONE_BYTE_STRING + CID_TWO_BYTE_STRING > 0);
    assert!(CID_GROWABLE_LIST + CID_LIST + CID_IMMUTABLE_LIST + CID_CLOSURE + CID_RECORD > 0);
    assert!(CID_UINT8_CLAMPED_LIST + CID_INT16_LIST + CID_UINT16_LIST + CID_INT32_LIST + CID_UINT32_LIST > 0);
    assert!(CID_UINT64_LIST + CID_FLOAT32_LIST + CID_FLOAT64_LIST + CID_INT32X4_LIST + CID_FLOAT32X4_LIST > 0);
};

/// The `[hash, input]` pairs of a class, in runtime memory.
pub type MethodTable = std::sync::Arc<[[i64; 2]]>;

/// Copies `n` pairs from a module constant.
///
/// # Safety
/// `pair_list` points to `2 * n` readable words.
unsafe fn copy_table(pair_list: *const i64, n: usize) -> MethodTable {
    // SAFETY: guaranteed by the caller.
    unsafe { std::slice::from_raw_parts(pair_list as *const [i64; 2], n) }.into()
}

/// Registers the method table of class `cid`: `n` pairs `[hash, input]` sorted by
/// hash, in a module constant, which is copied.
///
/// # Safety
/// `pair_list` points to `2 * n` readable words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_methods(cid: i64, pair_list: *const i64, n: i64) {
    // SAFETY: guaranteed by the caller.
    let t = unsafe { copy_table(pair_list, n as usize) };
    METHODS.with(|m| m.borrow_mut().insert(cid, t));
    mark_registered(cid);
}

/// Registers the method table of class `cid` through the function that returns it
/// (`df.mt.<library>.<Class>`, `{cid, n, [hash, input]…}`), if not registered yet.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_register_table(cid: i64, f: extern "C" fn() -> *const i64) {
    if already_registered(cid) && !REPUBLISHING.with(|r| r.get()) {
        return;
    }
    let t = f();
    // SAFETY: the table is a module constant: cid, n and the n pairs (from the
    // third word on), copied here.
    let table = unsafe { copy_table(t.add(2), *t.add(1) as usize) };
    METHODS.with(|m| m.borrow_mut().insert(cid, table));
    mark_registered(cid);
}

/// Up to which class id the [`REGISTERED`] vector applies: the program and SDK ids
/// are dense and small; the runtime's internal ones (the `Type` class,
/// 0x3FFF_FF01…) stay with the map.
const REGISTERED_UP_TO: usize = 1 << 16;

/// Publishes [`REGISTERED`] in the thread context (the inline allocation of
/// `dartforge_object_new_t` checks the registration there, `llvm/mod.rs`).
fn publish_registered(r: &[bool]) {
    crate::heap::CONTEXT.with(|c| {
        c.registered.set(r.as_ptr().cast());
        c.n_registered.set(r.len());
    });
}

/// Does class `cid` already have a registered table?
fn already_registered(cid: i64) -> bool {
    match usize::try_from(cid) {
        Ok(i) if i < REGISTERED_UP_TO => REGISTERED.with(|r| r.borrow().get(i).copied().unwrap_or(false)),
        _ => METHODS.with(|m| m.borrow().contains_key(&cid)),
    }
}

fn mark_registered(cid: i64) {
    if let Ok(i) = usize::try_from(cid)
        && i < REGISTERED_UP_TO
    {
        REGISTERED.with(|r| {
            let mut r = r.borrow_mut();
            if r.len() <= i {
                r.resize(i + 1, false);
            }
            r[i] = true;
            publish_registered(&r);
        });
    }
}

thread_local! {
    /// The registrations of a JIT reload's new generation are being redone: the
    /// method tables replace the ones already registered.
    static REPUBLISHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The publication of a new generation of the program (the JIT reload), at the
/// isolate's safepoint: extends the globals area to the generation's layout
/// (`area`, its `df.prepare_area` — now, with no frame at all on the stack, so
/// that a growth needing another allocation does not leave an old `%area` pointing
/// to the retired one); redoes the program's registrations (`register`, the
/// generation's `df.register.program`: names, subtype edges and method tables,
/// which now REPLACE those of the previous generation — a class may have gained
/// methods) and the RTI's supertype rules (`rti`). The registrations that add
/// (edges, rules) do not duplicate.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_publish_generation(
    area: Option<extern "C" fn()>,
    register: Option<extern "C" fn()>,
    rti: Option<extern "C" fn()>,
) {
    // The live objects of this isolate move to the new generation's layout (J03).
    apply_pending_migration();
    if let Some(f) = area {
        f();
    }
    REPUBLISHING.with(|r| r.set(true));
    // While the registrations are redone, every allocation goes through the
    // runtime, which registers again (`dartforge_object_new_t`).
    crate::heap::CONTEXT.with(|c| c.n_registered.set(0));
    if let Some(f) = register {
        f();
    }
    REPUBLISHING.with(|r| r.set(false));
    REGISTERED.with(|r| publish_registered(&r.borrow()));
    if let Some(f) = rti {
        f();
    }
    // Nothing of this isolate points to the memory of the previous generations anymore.
    if area.is_some() {
        forget_previous_area_generations();
    }
    // The method tables changed: the call-site caches (those of the new code and
    // those of what remains from previous generations, J04) look up again.
    empty_area_caches();
}

/// `dartforge_object_new` that registers the class's method table on the first
/// allocation (SDK from source).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_new_t(cid: i64, fields: i64, f: extern "C" fn() -> *const i64) -> i64 {
    dartforge_register_table(cid, f);
    dartforge_object_new(cid, fields)
}

/// The module's ABI check (docs/NATIVO-ESPACO-UNIFICADO.md §2.4, §3.6): the
/// runtime classes have fixed cids, and a module compiled with a numbering other
/// than this runtime's cannot run on it. `ids` is the module's table: the cids of
/// `layout::cid::SDK_CLASSES` in order, or the 32 positions of
/// [`FIXED_CIDS_BY_POSITION`] (a class that the loaded SDK does not have comes as
/// -1). If different: the process aborts with the message.
///
/// # Safety
/// `ids` points to `n` readable words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_cids(ids: *const i64, n: i64) {
    // SAFETY: the emitter passes a module constant with `n` words.
    let v = unsafe { std::slice::from_raw_parts(ids, usize::try_from(n).unwrap_or(0)) };
    let expected: Vec<i64> = if v.len() == crate::layout::cid::SDK_CLASSES.len() {
        crate::layout::cid::SDK_CLASSES.iter().map(|&(c, _, _)| i64::from(c)).collect()
    } else {
        FIXED_CIDS_BY_POSITION.iter().map(|&c| i64::from(c)).collect()
    };
    let checks = v.len() == expected.len() && v.iter().zip(&expected).all(|(&m, &r)| m == r || m == -1);
    if !checks {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = writeln!(
            std::io::stderr().lock(),
            "error: the module was compiled with another runtime class table (ABI): module {v:?}, runtime {expected:?}"
        );
        std::process::abort();
    }
}

/// The uniform entry of selector `hash` in the table of class `cid`, if it has one.
fn class_method(cid: i64, hash: i64) -> Option<usize> {
    METHODS.with(|m| {
        let m = m.borrow();
        let pair_list = m.get(&cid)?;
        let i = pair_list.binary_search_by(|pair| pair[0].cmp(&hash)).ok()?;
        Some(pair_list[i][1] as usize)
    })
}

/// The names of the named arguments, by the descriptor's hash (the same in every
/// module and every isolate).
fn argument_names() -> &'static std::sync::RwLock<HashMap<i64, String>> {
    static N: std::sync::OnceLock<std::sync::RwLock<HashMap<i64, String>>> = std::sync::OnceLock::new();
    N.get_or_init(Default::default)
}

/// Registers the name of a named argument that some descriptor of the module uses.
///
/// # Safety
/// `name` points to `len` UTF-8 bytes of a module constant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_argument_name(name: *const u8, len: i64) {
    // SAFETY: a module constant with `len` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(name, len as usize) };
    let name = String::from_utf8_lossy(bytes).into_owned();
    let h = name_hash(&name);
    argument_names().write().unwrap_or_else(|e| e.into_inner()).entry(h).or_insert(name);
}

/// 64-bit FNV-1a (`lower/closures.rs::hash_nome`).
fn name_hash(name: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h as i64
}

/// The entry of a selector that the receiver's class does not have: as in the VM,
/// the receiver's `noSuchMethod` receives the `Invocation` (`Object`'s throws
/// `NoSuchMethodError`; a class may override it and return a value).
extern "C" fn dartforge_nsm_selector(recv: i64, args: *const i64, desc: *const i64) -> i64 {
    let text = MISSING_SELECTOR.with(|s| s.borrow().clone());
    let (ty, name) = text.split_once(':').unwrap_or(("c", text.as_str()));
    let name = name.split_once('@').map_or(name, |(n, _)| n);
    let code = match ty {
        "g" => 1,
        "s" => 2,
        _ => 0,
    };
    // SAFETY: the descriptor and the argument vector are the call's; a call by
    // selector always carries the type tuple after the arguments (0 without type
    // arguments).
    let (value_list, hashes, tuple) = unsafe {
        let (npos, n_named) = (*desc as usize, *desc.add(1) as usize);
        (
            std::slice::from_raw_parts(args, npos + n_named).to_vec(),
            std::slice::from_raw_parts(desc.add(2), n_named).to_vec(),
            *args.add(npos + n_named),
        )
    };
    let npos = value_list.len() - hashes.len();
    let names: Vec<String> = {
        let table = argument_names().read().unwrap_or_else(|e| e.into_inner());
        hashes.iter().map(|h| table.get(h).cloned().unwrap_or_default()).collect()
    };
    let n = with_roots(&[recv], || HEAP.with(|h| h.borrow_mut().allocate_str(name)));
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    if let Some(r) = with_roots(&[n], || invoke_no_such_method(recv, code, n, &value_list, npos, &names, tuple)) {
        return r;
    }
    let error = with_roots(&[n], || dartforge_no_such_method_error_new(n));
    with_roots(&[error], || dartforge_exception_throw(error, 3));
    0
}

/// The type arguments of an RTI tuple (`L<…>`), or none.
fn tuple_arguments(tuple: i64) -> Vec<i64> {
    if tuple <= 0 {
        return Vec::new();
    }
    RTI.with(|u| match u.borrow().get_value(tuple) {
        Some(Type::Tuple(v)) => v.clone(),
        _ => Vec::new(),
    })
}

/// `receiver_obj.noSuchMethod(Invocation)` through `_dartforgeNoSuchMethod`
/// (`code`: 0 method, 1 getter, 2 setter): `value_list` are the positionals and
/// then the named ones, in the order of `names`. `None` without the SDK from source.
fn invoke_no_such_method(recv: i64, code: i64, name: i64, value_list: &[i64], npos: usize, names: &[&str], tuple: i64) -> Option<i64> {
    let f = helper("_dartforgeNoSuchMethod")?;
    let types = tuple_arguments(tuple);
    // Everything allocated here stays in a root frame until the call (each
    // allocation may collect).
    let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(value_list.len() + names.len() + types.len() + 7));
    let mut next_offset = 0;
    let mut root_it = |x: i64| {
        HEAP.with(|h| h.borrow_mut().set_root(frame, next_offset, x));
        next_offset += 1;
        x
    };
    root_it(recv);
    root_it(name);
    for &v in value_list {
        root_it(v);
    }
    let pos = root_it(dart_fixed_list(&value_list[..npos]));
    let text: Vec<i64> = names.iter().map(|t| root_it(HEAP.with(|h| h.borrow_mut().allocate_str(t)))).collect();
    let names = root_it(dart_fixed_list(&text));
    let vals = root_it(dart_fixed_list(&value_list[npos..]));
    let objects: Vec<i64> = types.iter().map(|&t| root_it(dartforge_rti_object_type(t))).collect();
    let types = root_it(dart_fixed_list(&objects));
    // SAFETY: registered by `dart:core` with the signature
    // (Object?, int, String, List, List, List, List) -> Object?.
    let g = { let dart_target: usize = f; move |a0: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64, a6: i64| -> i64 { dart_r7(dart_target, a0, a1, a2, a3, a4, a5, a6) } };
    let r = g(recv, code, name, pos, names, vals, types);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    Some(r)
}

/// The `noSuchMethod` forwarder that the compiler generates in the table of a class
/// with its own `noSuchMethod`, for an interface member without an implementation:
/// `env` has the `npos` positionals, the `n_named` named ones (all of them, with the
/// defaults) and then their names, in declaration order.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_forward_nsm(recv: i64, code: i64, name: i64, env: i64, npos: i64, n_named: i64, tuple: i64) -> i64 {
    let (npos, n_named) = (npos as usize, n_named as usize);
    // The forwarder's `_Contexto` (`lower/sdk_fonte.rs`, `gerar_encaminhador_nsm`)
    // keeps all the values as `Ref` (boxed by the compiler) and then the names.
    // Without any argument the context is `null` (an empty `AllocEnv` does not
    // allocate).
    let fields: Vec<i64> = HEAP.with(|h| {
        let h = h.borrow();
        if !crate::layout::is_object(env) {
            return Vec::new();
        }
        let n = h.object(env).map_or(0, |o| o.len());
        (0..n).map(|i| h.capture(env, i).0).collect()
    });
    if fields.len() != npos + 2 * n_named {
        return 0;
    }
    let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(3));
    let root_it = |i: usize, x: i64| HEAP.with(|h| h.borrow_mut().set_root(frame, i, x));
    root_it(0, recv);
    root_it(1, name);
    root_it(2, env);
    let value_list: Vec<i64> = fields[..npos + n_named].to_vec();
    let names: Vec<String> = HEAP.with(|h| {
        let h = h.borrow();
        fields[npos + n_named..].iter().map(|&t| h.text(t).map(|t| t.to_dart_string()).unwrap_or_default()).collect()
    });
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let r = invoke_no_such_method(recv, code, name, &value_list, npos, &names, tuple).unwrap_or(0);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    r
}

/// The entry of `recv.<selector>`, through the call site's cache (`cache[0]` =
/// class id + 1, `cache[1]` = entry).
///
/// # Safety
/// `cache` points to two writable words of the module; `name` to `len` readable
/// bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_selector(cache: *mut i64, recv: i64, hash: i64, name: *const u8, len: i64) -> usize {
    let cid = dartforge_value_class(recv);
    // SAFETY: the cache is a `[2 x i64]` private to the call site.
    unsafe {
        if *cache == cid.wrapping_add(1) {
            return *cache.add(1) as usize;
        }
    }
    let mut found_item = class_method(cid, hash);
    // The selector of the typed call (`tc:m`, `ts:x`; `lower/entrada_tipada.rs`) is
    // only in the table of the methods that have the typed entry: without it, the
    // usual entry (`c:m`, `s:x`), which also works — it checks more.
    // SAFETY: the name is a module constant with `len` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(name, len as usize) };
    let typed_one = bytes.first() == Some(&b't');
    if found_item.is_none() && typed_one {
        let without_t = String::from_utf8_lossy(&bytes[1..]);
        found_item = class_method(cid, name_hash(&without_t));
    }
    match found_item {
        Some(f) => {
            // SAFETY: see above.
            unsafe {
                *cache = cid.wrapping_add(1);
                *cache.add(1) = f as i64;
            }
            f
        }
        None => {
            let text = String::from_utf8_lossy(if typed_one { &bytes[1..] } else { bytes }).into_owned();
            if debug() {
                let class = CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned()).unwrap_or_default();
                eprintln!("[debug] missing selector: {text} in class {cid} {class}");
            }
            MISSING_SELECTOR.with(|s| *s.borrow_mut() = text);
            dartforge_nsm_selector as usize
        }
    }
}

/// A member of the SDK from source that the backend rejected (P5c): the program
/// reached it at run time. Never a wrong output: the message names the member and
/// the reason, and the process exits with 254 (the harness's "compile error" code).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_member_rejected(text: i64) {
    let t = HEAP.with(|heap| heap.borrow().text(text).map(|t| t.to_dart_string()).unwrap_or_default());
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = writeln!(std::io::stderr().lock(), "error: SDK member not supported by the native backend: {t}");
    std::process::exit(254);
}

/// `DARTFORGE_DEPURAR=1`: the runtime reports on stderr every thrown exception and
/// every selector that the receiver's class does not have.
fn debug() -> bool {
    thread_local! {
        static D: bool = std::env::var("DARTFORGE_DEPURAR").is_ok_and(|v| v == "1");
    }
    D.with(|d| *d)
}

thread_local! {
    /// The stack of Dart functions (`DARTFORGE_RASTRO=1` at compile time).
    static TRACE: RefCell<Vec<(usize, usize)>> = const { RefCell::new(Vec::new()) };
}

/// Entry of a function (debug call trace).
///
/// # Safety
/// `name` points to `len` bytes of a module constant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_call_trace_entry(name: *const u8, len: i64) {
    TRACE.with(|r| r.borrow_mut().push((name as usize, len as usize)));
}

/// Exit of a function (debug call trace).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_call_trace_exit() {
    TRACE.with(|r| {
        r.borrow_mut().pop();
    });
}

/// The current stack of Dart functions, from the innermost outwards.
fn show_trace() {
    TRACE.with(|r| {
        for &(p, n) in r.borrow().iter().rev().take(25) {
            // SAFETY: module constants registered by `dartforge_call_trace_entry`.
            let b = unsafe { std::slice::from_raw_parts(p as *const u8, n) };
            eprintln!("    at {}", String::from_utf8_lossy(b));
        }
    });
}

/// The migration of the object layouts of the last structural reload (J03), with
/// its epoch: per class, the old position of each new field (`-1`: none).
fn pending_migration() -> &'static std::sync::Mutex<Option<(u64, std::sync::Arc<HashMap<i64, Vec<i64>>>)>> {
    static M: std::sync::OnceLock<std::sync::Mutex<Option<(u64, std::sync::Arc<HashMap<i64, Vec<i64>>>)>>> =
        std::sync::OnceLock::new();
    M.get_or_init(Default::default)
}

/// Sets the migration of the publication in progress (J03): `data` is
/// `[n, (class, new_len, origin…)…]`. Called by the JIT at the main isolate's
/// safepoint, with the others stopped; each isolate applies it to its heap in
/// [`dartforge_publish_generation`].
///
/// # Safety
/// `data` points to `n` readable words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_set_migration(data: *const i64, n: i64) {
    // SAFETY: guaranteed by the caller; copied here.
    let v = unsafe { std::slice::from_raw_parts(data, usize::try_from(n).unwrap_or(0)) };
    let mut plan: HashMap<i64, Vec<i64>> = HashMap::default();
    let mut i = 1;
    for _ in 0..v.first().copied().unwrap_or(0) {
        let (Some(&class), Some(&len)) = (v.get(i), v.get(i + 1)) else { break };
        let len = usize::try_from(len).unwrap_or(0);
        let Some(origin) = v.get(i + 2..i + 2 + len) else { break };
        // The runtime classes (cid < 128) have a fixed layout, the same in every
        // generation: a plan that changes them is rejected (§2.13).
        if class < crate::layout::FIRST_FREE_CID {
            use std::io::Write;
            let _ = writeln!(std::io::stderr().lock(), "error: migration rejected for the runtime class {class}");
        } else {
            plan.insert(class, origin.to_vec());
        }
        i += 2 + len;
    }
    let epoch = crate::heap::LAYOUT_EPOCH.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
    *pending_migration().lock().unwrap_or_else(|e| e.into_inner()) = Some((epoch, std::sync::Arc::new(plan)));
}

/// Applies the pending migration to this isolate's heap, if it is not at its epoch
/// yet. A heap created after the migration is already born at the new epoch.
fn apply_pending_migration() {
    let current = crate::heap::LAYOUT_EPOCH.load(std::sync::atomic::Ordering::Acquire);
    let plan = pending_migration().lock().unwrap_or_else(|e| e.into_inner()).clone();
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        if h.layout_epoch >= current {
            return;
        }
        if let Some((epoch, plan)) = plan
            && epoch == current
        {
            h.migrate_instances(&plan);
        }
        h.layout_epoch = current;
    });
}
