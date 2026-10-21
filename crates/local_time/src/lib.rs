//! The local time zone of the machine, as the Dart VM computes it: the offset in
//! seconds east of UTC and the abbreviated name at an instant. Shared by the
//! native runtime (`DateTime.timeZoneOffset`, `timeZoneName`) and by the
//! executor of WebAssembly modules, which answers the host ABI's `time_zone`
//! with the same values a native executable would show.
#![allow(unsafe_code)]

/// The local time zone at instant `seconds` (since the epoch): offset in seconds
/// east of UTC and the abbreviated name — `localtime_r`, like the VM
/// (`OS::GetTimeZoneOffsetInSeconds`/`GetTimeZoneName`).
#[cfg(unix)]
pub fn local_time_zone(seconds: i64) -> (i64, String) {
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
        unsafe { std::ffi::CStr::from_ptr(tm.tm_zone) }
            .to_string_lossy()
            .into_owned()
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
pub fn local_time_zone(seconds: i64) -> (i64, String) {
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
        fn GetTimeZoneInformationForYear(
            year: u16,
            dtzi: *const u8,
            tz: *mut TimeZoneInformation,
        ) -> i32;
        fn FileTimeToSystemTime(ft: *const u64, st: *mut SystemTime) -> i32;
        fn SystemTimeToTzSpecificLocalTime(
            tz: *const TimeZoneInformation,
            utc: *const SystemTime,
            local: *mut SystemTime,
        ) -> i32;
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
                let dst_bias = if mode == TIME_ZONE_ID_INVALID {
                    -60 * 60
                } else {
                    current.daylight_bias as i64 * 60
                };
                d -= dst_bias;
            }
            d
        }
    };
    let name_str = match in_dst {
        _ if mode == TIME_ZONE_ID_INVALID => String::new(),
        None => String::new(),
        Some(dst) => {
            let name_str = if dst {
                &current.daylight_name
            } else {
                &current.standard_name
            };
            let end = name_str
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(name_str.len());
            String::from_utf16_lossy(&name_str[..end])
        }
    };
    (offset, name_str)
}

/// Targets without a supported system zone database: UTC.
#[cfg(not(any(unix, windows)))]
pub fn local_time_zone(_seconds: i64) -> (i64, String) {
    (0, "UTC".to_string())
}
