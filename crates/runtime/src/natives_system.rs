// Native runtime: natives of the SDK from source that talk to the system — clock,
// time zone and entropy (the VM's `runtime/lib/date.cc`, `stopwatch.cc`,
// `math.cc`). Table in `crates/emit_native/src/nativos.rs`.

/// The starting instant of the process's monotonic clock.
fn monotonic_start() -> std::time::Instant {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *START.get_or_init(std::time::Instant::now)
}

/// `Stopwatch_frequency`: ticks per second of the `Stopwatch_now` clock
/// (nanoseconds, like the VM on Linux and macOS).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stopwatch_frequency() -> i64 {
    1_000_000_000
}

/// `Stopwatch_now`: the monotonic clock, in nanoseconds.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stopwatch_now() -> i64 {
    monotonic_start().elapsed().as_nanos() as i64
}

/// `DateTime_currentTimeMicros`: microseconds since the Unix epoch (UTC).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DateTime_currentTimeMicros() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_micros() as i64,
        Err(e) => -(e.duration().as_micros() as i64),
    }
}

/// The local time zone at instant `seconds` (since the epoch): offset in seconds
/// east of UTC and the abbreviated name — `localtime_r`, like the VM
/// (`OS::GetTimeZoneOffsetInSeconds`/`GetTimeZoneName`).
#[cfg(unix)]
fn local_timezone(seconds: i64) -> (i64, String) {
    use std::os::raw::{c_char, c_int, c_long};
    #[repr(C)]
    struct Tm {
        tm_sec: c_int,
        tm_min: c_int,
        tm_hour: c_int,
        tm_mday: c_int,
        tm_mon: c_int,
        tm_year: c_int,
        tm_wday: c_int,
        tm_yday: c_int,
        tm_isdst: c_int,
        tm_gmtoff: c_long,
        tm_zone: *const c_char,
    }
    unsafe extern "C" {
        fn localtime_r(t: *const i64, tm: *mut Tm) -> *mut Tm;
        fn tzset();
    }
    let mut tm = Tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    // SAFETY: `tzset` only reads the environment; `localtime_r` writes into `tm`,
    // which is valid, and returns null on error.
    let ok = unsafe {
        tzset();
        !localtime_r(&seconds, &mut tm).is_null()
    };
    if !ok {
        return (0, String::new());
    }
    let name_str = if tm.tm_zone.is_null() {
        String::new()
    } else {
        // SAFETY: `tm_zone` points to a static C string of libc.
        unsafe { std::ffi::CStr::from_ptr(tm.tm_zone) }.to_string_lossy().into_owned()
    };
    (tm.tm_gmtoff as i64, name_str)
}

/// The local time zone on Windows at instant `seconds`, like the VM
/// (runtime/vm/os_win.cc: `LocalTime`, `GetTimeZoneOffsetInSeconds`,
/// `GetDaylightSavingBiasInSeconds`, `GetTimeZoneName`):
///
/// * the daylight saving time of the INSTANT comes from the zone's rules for its
///   year (`GetTimeZoneInformationForYear` + `SystemTimeToTzSpecificLocalTime`,
///   converted twice — with the daylight bias and with it zeroed; different hours
///   = in daylight saving time);
/// * the offset is the CRT's `-_timezone` (after `_tzset`), minus the system's
///   CURRENT daylight bias when the instant is in daylight saving time;
/// * the name is the current zone's `DaylightName`/`StandardName`.
///
/// It used to use only the current state (`GetTimeZoneInformation`), for any
/// instant: `DateTime(2019)` in the Brasília zone gave 00:00 where the VM gives
/// 01:00 (midnight fell into that year's daylight saving time), and every summer
/// `DateTime` of a past year came out with the wrong offset.
#[cfg(windows)]
fn local_timezone(seconds: i64) -> (i64, String) {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct SystemTime {
        year: u16,
        month: u16,
        weekday: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        millisecond: u16,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct TimeZoneInformation {
        bias: i32,
        standard_name: [u16; 32],
        standard_date: SystemTime,
        standard_bias: i32,
        daylight_name: [u16; 32],
        daylight_date: SystemTime,
        daylight_bias: i32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetTimeZoneInformation(tz: *mut TimeZoneInformation) -> u32;
        fn GetTimeZoneInformationForYear(year: u16, dtzi: *const u8, tz: *mut TimeZoneInformation) -> i32;
        fn FileTimeToSystemTime(ft: *const u64, st: *mut SystemTime) -> i32;
        fn SystemTimeToTzSpecificLocalTime(tz: *const TimeZoneInformation, utc: *const SystemTime, local: *mut SystemTime) -> i32;
    }
    unsafe extern "C" {
        fn _tzset();
        fn _get_timezone(seconds: *mut i32) -> i32;
    }
    const TIME_ZONE_ID_INVALID: u32 = 0xFFFF_FFFF;
    // 1601-01-01 → 1970-01-01, in units of 100 ns (`kTimeEpoc`).
    const FILETIME_EPOCH: i64 = 116_444_736_000_000_000;

    // `LocalTime`: is the instant in daylight saving time? (`None`: the conversion
    // failed.)
    // SAFETY: the structures are plain data (zeros are valid) and the functions
    // write into them.
    let in_dst = (|| unsafe {
        let ft = FILETIME_EPOCH.checked_add(seconds.checked_mul(10_000_000)?)? as u64;
        let mut utc: SystemTime = std::mem::zeroed();
        if FileTimeToSystemTime(&ft, &mut utc) == 0 {
            return None;
        }
        let mut tz: TimeZoneInformation = std::mem::zeroed();
        if GetTimeZoneInformationForYear(utc.year, std::ptr::null(), &mut tz) == 0 {
            return None;
        }
        let mut local: SystemTime = std::mem::zeroed();
        if SystemTimeToTzSpecificLocalTime(&tz, &utc, &mut local) == 0 {
            return None;
        }
        if tz.daylight_bias == 0 {
            return Some(false);
        }
        let with_bias = local.hour;
        tz.daylight_bias = 0;
        if SystemTimeToTzSpecificLocalTime(&tz, &utc, &mut local) == 0 {
            return None;
        }
        Some(with_bias != local.hour)
    })();
    // SAFETY: likewise.
    let mut current: TimeZoneInformation = unsafe { std::mem::zeroed() };
    let mode = unsafe { GetTimeZoneInformation(&mut current) };
    let offset = match in_dst {
        None => 0, // "Return zero like V8 does."
        Some(dst) => {
            let mut west = 0i32;
            // SAFETY: `_tzset` only reads the environment and the system's zone;
            // `_get_timezone` writes into the integer.
            unsafe {
                _tzset();
                _get_timezone(&mut west);
            }
            let mut d = -(west as i64);
            if dst {
                let dst_bias =
                    if mode == TIME_ZONE_ID_INVALID { -60 * 60 } else { current.daylight_bias as i64 * 60 };
                d -= dst_bias;
            }
            d
        }
    };
    let name_str = match in_dst {
        _ if mode == TIME_ZONE_ID_INVALID => String::new(),
        None => String::new(),
        Some(dst) => {
            let name_str = if dst { &current.daylight_name } else { &current.standard_name };
            let end = name_str.iter().position(|c| *c == 0).unwrap_or(name_str.len());
            String::from_utf16_lossy(&name_str[..end])
        }
    };
    (offset, name_str)
}

/// `DateTime_timeZoneOffsetInSeconds`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DateTime_timeZoneOffsetInSeconds(seconds: i64) -> i64 {
    local_timezone(seconds).0
}

/// `DateTime_timeZoneName`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DateTime_timeZoneName(seconds: i64) -> i64 {
    HEAP.with(|h| h.borrow_mut().allocate_str(&local_timezone(seconds).1))
}

/// System entropy: Rust's `RandomState` is seeded by the operating system's
/// secure generator in each process; each call mixes in a counter, so
/// consecutive values differ.
fn entropy() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    h.finish()
}

/// `Random_initialSeed`: the seed of `Random()` without an argument.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Random_initialSeed() -> i64 {
    entropy() as i64
}

/// `SecureRandom_getBytes(n)`: `n` (1 to 8) random bytes in an integer.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SecureRandom_getBytes(n: i64) -> i64 {
    let v = entropy();
    if n >= 8 { v as i64 } else { (v & ((1u64 << (8 * n.max(0))) - 1)) as i64 }
}

/// `Uri_isWindowsPlatform`: whether the host system is Windows (`Uri.file` and
/// `Uri.base` use its path rules).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Uri_isWindowsPlatform() -> u8 {
    u8::from(cfg!(windows))
}

// ---------------------------------------------------------------------------
// `Object` and weak references (received from `natives_lists.rs`,
// docs/NATIVO-ESPACO-UNIFICADO.md §4.9). The identity hash (`Object_getHash`)
// stays with the boxes (P2).

/// `Object.==` (`Object_equals`): identity.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_equals(this: i64, other_one: i64) -> u8 {
    dartforge_identical(this, other_one)
}

/// `Object.toString()` (`Object_toString`): `Instance of 'Class'`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_toString(this: i64) -> i64 {
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
        return dartforge_nativo_Double_toString(d);
    }
    if let Some(t) = simd_string(this) {
        return HEAP.with(|h| h.borrow_mut().allocate_str(&t));
    }
    let cid = dartforge_value_class(this);
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
pub extern "C" fn dartforge_nativo_Object_haveSameRuntimeType(a: i64, b: i64) -> u8 {
    u8::from(dartforge_value_class(a) == dartforge_value_class(b))
}

/// `Object.runtimeType`: uses the RTI universe, including reified type arguments,
/// and returns the isolate's canonical `Type` object.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_runtimeType(this: i64) -> i64 {
    dartforge_rti_objeto_tipo(dartforge_rti_do_valor(this))
}

// Weak references and ephemerons (`WeakReference`, `Expando`): the state lives in
// the heap's `weak_refs`/`ephemerons` tables, by handle, which the collection
// purges using the mark bit (`heap.rs`).

/// `WeakReference.target` (`WeakReference_getTarget`): the target, or null if it
/// was collected.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakReference_getTarget(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().weak_refs.get(&this).copied().unwrap_or(0))
}

/// `_WeakReference._target =` (`WeakReference_setTarget`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakReference_setTarget(this: i64, target: i64) {
    HEAP.with(|h| h.borrow_mut().weak_refs.insert(this, target));
}

/// `_WeakProperty.key` (`WeakProperty_getKey`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_getKey(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().ephemerons.get(&this).map_or(0, |p| p.0))
}

/// `_WeakProperty.key =` (`WeakProperty_setKey`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_setKey(this: i64, key: i64) {
    HEAP.with(|h| h.borrow_mut().ephemerons.entry(this).or_insert((0, 0)).0 = key);
}

/// `_WeakProperty.value` (`WeakProperty_getValue`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_getValue(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().ephemerons.get(&this).map_or(0, |p| p.1))
}

/// `_WeakProperty.value =` (`WeakProperty_setValue`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_setValue(this: i64, value: i64) {
    HEAP.with(|h| h.borrow_mut().ephemerons.entry(this).or_insert((0, 0)).1 = value);
}
