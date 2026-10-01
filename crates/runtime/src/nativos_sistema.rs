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

/// O fuso local no Windows no instante `segundos`, como a VM
/// (runtime/vm/os_win.cc: `LocalTime`, `GetTimeZoneOffsetInSeconds`,
/// `GetDaylightSavingBiasInSeconds`, `GetTimeZoneName`):
///
/// * o horário de verão do INSTANTE vem das regras do fuso para o ano dele
///   (`GetTimeZoneInformationForYear` + `SystemTimeToTzSpecificLocalTime`,
///   convertido duas vezes — com o viés de verão e com ele zerado; horas
///   diferentes = em horário de verão);
/// * o deslocamento é `-_timezone` da CRT (depois do `_tzset`), menos o viés
///   de verão ATUAL do sistema quando o instante está em horário de verão;
/// * o nome é o `DaylightName`/`StandardName` do fuso atual.
///
/// Antes usava só o estado de agora (`GetTimeZoneInformation`), para
/// qualquer instante: `DateTime(2019)` no fuso de Brasília dava 00:00 onde a
/// VM dá 01:00 (a meia-noite caiu no horário de verão daquele ano), e todo
/// `DateTime` de verão de um ano passado saía com o deslocamento errado.
#[cfg(windows)]
fn fuso_local(segundos: i64) -> (i64, String) {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct SystemTime {
        ano: u16,
        mes: u16,
        dia_da_semana: u16,
        dia: u16,
        hora: u16,
        minuto: u16,
        segundo: u16,
        milissegundo: u16,
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
        fn GetTimeZoneInformationForYear(ano: u16, dtzi: *const u8, tz: *mut TimeZoneInformation) -> i32;
        fn FileTimeToSystemTime(ft: *const u64, st: *mut SystemTime) -> i32;
        fn SystemTimeToTzSpecificLocalTime(tz: *const TimeZoneInformation, utc: *const SystemTime, local: *mut SystemTime) -> i32;
    }
    unsafe extern "C" {
        fn _tzset();
        fn _get_timezone(segundos: *mut i32) -> i32;
    }
    const TIME_ZONE_ID_INVALID: u32 = 0xFFFF_FFFF;
    // 1601-01-01 → 1970-01-01, em unidades de 100 ns (`kTimeEpoc`).
    const EPOCA_DO_FILETIME: i64 = 116_444_736_000_000_000;

    // `LocalTime`: o instante em horário de verão? (`None`: a conversão falhou.)
    // SAFETY: as estruturas são só dados (zeros são válidos) e as funções
    // escrevem nelas.
    let em_verao = (|| unsafe {
        let ft = EPOCA_DO_FILETIME.checked_add(segundos.checked_mul(10_000_000)?)? as u64;
        let mut utc: SystemTime = std::mem::zeroed();
        if FileTimeToSystemTime(&ft, &mut utc) == 0 {
            return None;
        }
        let mut tz: TimeZoneInformation = std::mem::zeroed();
        if GetTimeZoneInformationForYear(utc.ano, std::ptr::null(), &mut tz) == 0 {
            return None;
        }
        let mut local: SystemTime = std::mem::zeroed();
        if SystemTimeToTzSpecificLocalTime(&tz, &utc, &mut local) == 0 {
            return None;
        }
        if tz.daylight_bias == 0 {
            return Some(false);
        }
        let com_vies = local.hora;
        tz.daylight_bias = 0;
        if SystemTimeToTzSpecificLocalTime(&tz, &utc, &mut local) == 0 {
            return None;
        }
        Some(com_vies != local.hora)
    })();
    // SAFETY: idem.
    let mut atual: TimeZoneInformation = unsafe { std::mem::zeroed() };
    let modo = unsafe { GetTimeZoneInformation(&mut atual) };
    let deslocamento = match em_verao {
        None => 0, // "Return zero like V8 does."
        Some(verao) => {
            let mut oeste = 0i32;
            // SAFETY: `_tzset` só lê o ambiente e o fuso do sistema;
            // `_get_timezone` escreve no inteiro.
            unsafe {
                _tzset();
                _get_timezone(&mut oeste);
            }
            let mut d = -(oeste as i64);
            if verao {
                let vies_de_verao =
                    if modo == TIME_ZONE_ID_INVALID { -60 * 60 } else { atual.daylight_bias as i64 * 60 };
                d -= vies_de_verao;
            }
            d
        }
    };
    let nome = match em_verao {
        _ if modo == TIME_ZONE_ID_INVALID => String::new(),
        None => String::new(),
        Some(verao) => {
            let nome = if verao { &atual.daylight_name } else { &atual.standard_name };
            let fim = nome.iter().position(|c| *c == 0).unwrap_or(nome.len());
            String::from_utf16_lossy(&nome[..fim])
        }
    };
    (deslocamento, nome)
}

/// `DateTime_timeZoneOffsetInSeconds`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DateTime_timeZoneOffsetInSeconds(segundos: i64) -> i64 {
    fuso_local(segundos).0
}

/// `DateTime_timeZoneName`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DateTime_timeZoneName(segundos: i64) -> i64 {
    HEAP.with(|h| h.borrow_mut().alocar_str(&fuso_local(segundos).1))
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

/// `Uri_isWindowsPlatform`: se o sistema hospedeiro é o Windows (o
/// `Uri.file` e o `Uri.base` usam as regras de caminho dele).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Uri_isWindowsPlatform() -> u8 {
    u8::from(cfg!(windows))
}

// ---------------------------------------------------------------------------
// `Object` e as referências fracas (recebidos de `nativos_listas.rs`,
// docs/NATIVO-ESPACO-UNIFICADO.md §4.9). O hash de identidade
// (`Object_getHash`) fica com as caixas (P2).

/// `Object.==` (`Object_equals`): identidade.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_equals(this: i64, outro: i64) -> u8 {
    dartforge_identical(this, outro)
}

/// `Object.toString()` (`Object_toString`): `Instance of 'Classe'`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_toString(this: i64) -> i64 {
    // O `Object_toString` da VM também escreve os números (o `_Mint` e o
    // `_Double` não têm `toString` próprio na fonte): pelo cid, sem desencaixar
    // o que não é número.
    let (inteiro, real) = HEAP.with(|h| {
        let h = h.borrow();
        (h.int_de(this), h.double_de(this))
    });
    if let Some(i) = inteiro {
        return dartforge_to_string_i64(i);
    }
    if let Some(d) = real {
        return dartforge_nativo_Double_toString(d);
    }
    if let Some(t) = texto_simd(this) {
        return HEAP.with(|h| h.borrow_mut().alocar_str(&t));
    }
    let cid = dartforge_value_class(this);
    // Genérica com argumentos reificados: o nome inclui os argumentos
    // (`Instance of 'Caixa<int>'`), como a VM; sem argumentos, o nome
    // registrado da classe.
    let nome = texto_com_argumentos(this)
        .unwrap_or_else(|| CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned()).unwrap_or_default());
    HEAP.with(|h| h.borrow_mut().alocar_str(&format!("Instance of '{nome}'")))
}

/// `Object._haveSameRuntimeType(a, b)`: a mesma classe (os argumentos de
/// tipo ficam para a RTI).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_haveSameRuntimeType(a: i64, b: i64) -> u8 {
    u8::from(dartforge_value_class(a) == dartforge_value_class(b))
}

/// `Object.runtimeType`: usa o universo RTI, inclusive argumentos de tipo
/// reificados, e devolve o objeto `Type` canônico do isolado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_runtimeType(this: i64) -> i64 {
    dartforge_rti_objeto_tipo(dartforge_rti_do_valor(this))
}

// Referências fracas e efêmeros (`WeakReference`, `Expando`): o estado mora
// nas tabelas `fracas`/`efemeros` do heap, por handle, que a coleta purga
// pelo bit de marca (`heap.rs`).

/// `WeakReference.target` (`WeakReference_getTarget`): o alvo, ou null se
/// foi coletado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakReference_getTarget(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().fracas.get(&this).copied().unwrap_or(0))
}

/// `_WeakReference._target =` (`WeakReference_setTarget`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakReference_setTarget(this: i64, alvo: i64) {
    HEAP.with(|h| h.borrow_mut().fracas.insert(this, alvo));
}

/// `_WeakProperty.key` (`WeakProperty_getKey`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_getKey(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().efemeros.get(&this).map_or(0, |p| p.0))
}

/// `_WeakProperty.key =` (`WeakProperty_setKey`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_setKey(this: i64, chave: i64) {
    HEAP.with(|h| h.borrow_mut().efemeros.entry(this).or_insert((0, 0)).0 = chave);
}

/// `_WeakProperty.value` (`WeakProperty_getValue`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_getValue(this: i64) -> i64 {
    HEAP.with(|h| h.borrow().efemeros.get(&this).map_or(0, |p| p.1))
}

/// `_WeakProperty.value =` (`WeakProperty_setValue`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_WeakProperty_setValue(this: i64, valor: i64) {
    HEAP.with(|h| h.borrow_mut().efemeros.entry(this).or_insert((0, 0)).1 = valor);
}
