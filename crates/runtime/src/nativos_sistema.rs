// Runtime nativo: natives do SDK da fonte que falam com o sistema — relógio,
// fuso horário e entropia (`runtime/lib/date.cc`, `stopwatch.cc`,
// `math.cc` da VM). Tabela em `crates/emit_native/src/nativos.rs`.

/// O instante de partida do relógio monotônico do processo.
fn inicio_monotonico() -> std::time::Instant {
    static INICIO: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *INICIO.get_or_init(std::time::Instant::now)
}

/// `Stopwatch_frequency`: tiques por segundo do relógio de `Stopwatch_now`
/// (nanossegundos, como a VM no Linux e no macOS).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stopwatch_frequency() -> i64 {
    1_000_000_000
}

/// `Stopwatch_now`: o relógio monotônico, em nanossegundos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stopwatch_now() -> i64 {
    inicio_monotonico().elapsed().as_nanos() as i64
}

/// `DateTime_currentTimeMicros`: microssegundos desde a época Unix (UTC).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DateTime_currentTimeMicros() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_micros() as i64,
        Err(e) => -(e.duration().as_micros() as i64),
    }
}

/// O fuso local no instante `segundos` (desde a época): deslocamento em
/// segundos a leste de UTC e o nome abreviado — `localtime_r`, como a VM
/// (`OS::GetTimeZoneOffsetInSeconds`/`GetTimeZoneName`).
#[cfg(unix)]
fn fuso_local(segundos: i64) -> (i64, String) {
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
    // SAFETY: `tzset` só lê o ambiente; `localtime_r` escreve em `tm`, que é
    // válido, e devolve nulo em erro.
    let ok = unsafe {
        tzset();
        !localtime_r(&segundos, &mut tm).is_null()
    };
    if !ok {
        return (0, String::new());
    }
    let nome = if tm.tm_zone.is_null() {
        String::new()
    } else {
        // SAFETY: `tm_zone` aponta para uma string C estática da libc.
        unsafe { std::ffi::CStr::from_ptr(tm.tm_zone) }.to_string_lossy().into_owned()
    };
    (tm.tm_gmtoff as i64, nome)
}

/// O fuso local no Windows (`GetTimeZoneInformation`): o viés e o nome do
/// horário padrão ou de verão, como a VM (`os_win.cc`).
#[cfg(windows)]
fn fuso_local(_segundos: i64) -> (i64, String) {
    #[repr(C)]
    struct SystemTime {
        campos: [u16; 8],
    }
    #[repr(C)]
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
    }
    // SAFETY: a estrutura é só dados (zeros são válidos) e a função escreve
    // nela.
    let mut tz: TimeZoneInformation = unsafe { std::mem::zeroed() };
    let modo = unsafe { GetTimeZoneInformation(&mut tz) };
    let verao = modo == 2;
    let vies = tz.bias + if verao { tz.daylight_bias } else { tz.standard_bias };
    let nome = if verao { &tz.daylight_name } else { &tz.standard_name };
    let fim = nome.iter().position(|c| *c == 0).unwrap_or(nome.len());
    (-(vies as i64) * 60, String::from_utf16_lossy(&nome[..fim]))
}

/// `DateTime_timeZoneOffsetInSeconds`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DateTime_timeZoneOffsetInSeconds(segundos: i64) -> i64 {
    fuso_local(segundos).0
}

/// `DateTime_timeZoneName`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DateTime_timeZoneName(segundos: i64) -> i64 {
    alocar_str(&fuso_local(segundos).1)
}

/// Entropia do sistema: o `RandomState` do Rust é semeado pelo gerador
/// seguro do sistema operacional em cada processo; cada chamada mistura um
/// contador, então valores seguidos diferem.
fn entropia() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    static CONTADOR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(CONTADOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    h.finish()
}

/// `Random_initialSeed`: a semente de `Random()` sem argumento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Random_initialSeed() -> i64 {
    entropia() as i64
}

/// `SecureRandom_getBytes(n)`: `n` (1 a 8) bytes aleatórios num inteiro.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SecureRandom_getBytes(n: i64) -> i64 {
    let v = entropia();
    if n >= 8 { v as i64 } else { (v & ((1u64 << (8 * n.max(0))) - 1)) as i64 }
}
