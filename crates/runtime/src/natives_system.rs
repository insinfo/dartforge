// Native runtime: natives of the SDK from source that talk to the system — clock,
// time zone and entropy (the VM's `runtime/lib/date.cc`, `stopwatch.cc`,
// `math.cc`). Table in `crates/emit_native/src/nativos.rs`.

/// The starting instant of the process's monotonic clock.
fn monotonic_start() -> crate::clock::Instant {
    static START: std::sync::OnceLock<crate::clock::Instant> = std::sync::OnceLock::new();
    *START.get_or_init(crate::clock::Instant::now)
}

/// `Stopwatch_frequency`: ticks per second of the `Stopwatch_now` clock
/// (nanoseconds, like the VM on Linux and macOS).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stopwatch_frequency() -> i64 {
    1_000_000_000
}

/// `Stopwatch_now`: the monotonic clock, in nanoseconds.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stopwatch_now() -> i64 {
    monotonic_start().elapsed().as_nanos() as i64
}

/// `DateTime_currentTimeMicros`: microseconds since the Unix epoch (UTC).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DateTime_currentTimeMicros() -> i64 {
    crate::clock::wall_clock_micros()
}

/// The local time zone at instant `seconds` (since the epoch): offset in seconds
/// east of UTC and the abbreviated name, like the VM (`dartforge-local-time`).
#[cfg(not(target_arch = "wasm32"))]
fn local_timezone(seconds: i64) -> (i64, String) {
    dartforge_local_time::local_time_zone(seconds)
}

/// The local time zone of a WebAssembly module: the host's (in a browser, the
/// page's `Intl`/`Date` zone, which is also what dart2wasm reports).
#[cfg(target_arch = "wasm32")]
fn local_timezone(seconds: i64) -> (i64, String) {
    crate::host::time_zone(seconds)
}

/// `DateTime_timeZoneOffsetInSeconds`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DateTime_timeZoneOffsetInSeconds(seconds: i64) -> i64 {
    local_timezone(seconds).0
}

/// `DateTime_timeZoneName`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DateTime_timeZoneName(seconds: i64) -> i64 {
    HEAP.with(|h| h.borrow_mut().allocate_str(&local_timezone(seconds).1))
}

/// System entropy: Rust's `RandomState` is seeded by the operating system's
/// secure generator in each process; each call mixes in a counter, so
/// consecutive values differ.
#[cfg(not(target_arch = "wasm32"))]
fn entropy() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    h.finish()
}

/// System entropy on WebAssembly: the host's cryptographic generator (`std`'s
/// `RandomState` has no source there and would repeat).
#[cfg(target_arch = "wasm32")]
fn entropy() -> u64 {
    let mut bytes = [0u8; 8];
    crate::host::random_fill(&mut bytes);
    u64::from_le_bytes(bytes)
}

/// `Random_initialSeed`: the seed of `Random()` without an argument.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Random_initialSeed() -> i64 {
    entropy() as i64
}

/// `SecureRandom_getBytes(n)`: `n` (1 to 8) random bytes in an integer.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SecureRandom_getBytes(n: i64) -> i64 {
    let v = entropy();
    if n >= 8 { v as i64 } else { (v & ((1u64 << (8 * n.max(0))) - 1)) as i64 }
}

/// `Uri_isWindowsPlatform`: whether the host system is Windows (`Uri.file` and
/// `Uri.base` use its path rules).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Uri_isWindowsPlatform() -> u8 {
    u8::from(cfg!(windows))
}

// ---------------------------------------------------------------------------
// `Object` and weak references (received from `natives_lists.rs`,
// docs/NATIVO-ESPACO-UNIFICADO.md §4.9). The identity hash (`Object_getHash`)
// stays with the boxes (P2).

/// `Object.==` (`Object_equals`): identity.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Object_equals(this: i64, other_one: i64) -> u8 {
    dartforge_identical(this, other_one)
}

/// `Object.toString()` (`Object_toString`): `Instance of 'Class'`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Object_toString(this: i64) -> i64 {
    // The VM's `Object_toString` also writes numbers (`_Mint` and `_Double` have
    // no `toString` of their own in the source): by cid, without unboxing what is
    // not a number.
    let (integer, real) = HEAP.with(|h| {
        let h = h.borrow();
        (h.int_of(this), h.double_of(this))
    });
    if let Some(i) = integer {
        return dartforge_to_string_i64(i);
    }
    if let Some(d) = real {
        return dartforge_native_Double_toString(d);
    }
    if let Some(t) = simd_string(this) {
        return HEAP.with(|h| h.borrow_mut().allocate_str(&t));
    }
    let cid = dartforge_value_class(this);
    // A closure: the VM's `Closure::ToCString` — its signature and, for a
    // tear-off, the function (`from Function 'name':[ static].`).
    if cid == i64::from(crate::layout::cid::CLOSURE) {
        // An instantiation (`f<int>`) is described by the function it
        // instantiates.
        let code = HEAP.with(|h| {
            let h = h.borrow();
            let c = h.closure(this)?;
            let mut code = c.code;
            let mut inner = instantiation_parts(&h, &c).map(|(i, _)| i);
            while let Some(i) = inner {
                let ci = h.closure(i)?;
                code = ci.code;
                inner = instantiation_parts(&h, &ci).map(|(i, _)| i);
            }
            Some(code)
        });
        let signature = type_text_of_value(this);
        let from = code
            .and_then(|code| TEAROFF_NAMES.with(|m| m.borrow().get(&code).cloned()))
            .map(|(name, is_static)| {
                format!(" from Function '{name}':{}.", if is_static { " static" } else { "" })
            })
            .unwrap_or_default();
        return HEAP.with(|h| h.borrow_mut().allocate_str(&format!("Closure: {signature}{from}")));
    }
    // A generic with reified arguments: the name includes the arguments
    // (`Instance of 'Caixa<int>'`), like the VM; without arguments, the class's
    // registered name.
    let name_str = text_with_arguments(this)
        .unwrap_or_else(|| CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned()).unwrap_or_default());
    HEAP.with(|h| h.borrow_mut().allocate_str(&format!("Instance of '{name_str}'")))
}

/// `Object._haveSameRuntimeType(a, b)`: the same class (the type arguments are
/// left to the RTI).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Object_haveSameRuntimeType(a: i64, b: i64) -> u8 {
    u8::from(dartforge_value_class(a) == dartforge_value_class(b))
}

/// `Object.runtimeType`: uses the RTI universe, including reified type arguments,
/// and returns the isolate's canonical `Type` object.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Object_runtimeType(this: i64) -> i64 {
    dartforge_rti_object_type(dartforge_rti_of_value(this))
}

// Weak references and ephemerons (`WeakReference`, `Expando`): the state lives in
// the heap's `weak_refs`/`ephemerons` tables, by handle, which the collection
// purges using the mark bit (`heap.rs`).

/// `WeakReference.target` (`WeakReference_getTarget`): the target, or null if it
/// was collected.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_WeakReference_getTarget(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().weak_refs.get(&this).copied().unwrap_or(0))
}

/// `_WeakReference._target =` (`WeakReference_setTarget`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_WeakReference_setTarget(this: i64, target: i64) {
    HEAP.with(|h| h.borrow_mut().weak_refs.insert(this, target));
}

/// `_WeakProperty.key` (`WeakProperty_getKey`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_WeakProperty_getKey(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().ephemerons.get(&this).map_or(0, |p| p.0))
}

/// `_WeakProperty.key =` (`WeakProperty_setKey`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_WeakProperty_setKey(this: i64, key: i64) {
    HEAP.with(|h| h.borrow_mut().ephemerons.entry(this).or_insert((0, 0)).0 = key);
}

/// `_WeakProperty.value` (`WeakProperty_getValue`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_WeakProperty_getValue(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().ephemerons.get(&this).map_or(0, |p| p.1))
}

/// `_WeakProperty.value =` (`WeakProperty_setValue`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_WeakProperty_setValue(this: i64, value: i64) {
    HEAP.with(|h| h.borrow_mut().ephemerons.entry(this).or_insert((0, 0)).1 = value);
}
