// Runtime nativo: o `dart:io` da VM — plataforma, terminal e processo
// (`runtime/bin/platform*.cc`, `stdio*.cc`, `crypto*.cc` e os natives
// simples de `process.cc`): `Platform`, `Stdin`/`Stdout`, `exit`,
// `exitCode`, `sleep`, `pid` e os bytes aleatórios do sistema.

// ---------------------------------------------------------------------------
// Bytes aleatórios do sistema (`Crypto::GetRandomBytes`).

/// Preenche `dest` com bytes do gerador seguro do sistema:
/// `getentropy` no Unix (em blocos de 256, o máximo por chamada) e
/// `BCryptGenRandom` no Windows.
#[cfg(unix)]
fn system_random_bytes(dest: &mut [u8]) -> IoResult<()> {
    unsafe extern "C" {
        fn getentropy(buf: *mut std::ffi::c_void, n: usize) -> i32;
    }
    for block in dest.chunks_mut(256) {
        // SAFETY: `block` é um trecho gravável de até 256 bytes.
        if unsafe { getentropy(block.as_mut_ptr().cast(), block.len()) } != 0 {
            return Err(OsError::de(&std::io::Error::last_os_error()));
        }
    }
    Ok(())
}
#[cfg(windows)]
fn system_random_bytes(dest: &mut [u8]) -> IoResult<()> {
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptGenRandom(alg: *mut std::ffi::c_void, buf: *mut u8, n: u32, flags: u32) -> i32;
    }
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 2;
    for block in dest.chunks_mut(u32::MAX as usize) {
        // SAFETY: `block` é gravável com o tamanho passado.
        let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), block.as_mut_ptr(), block.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
        if status != 0 {
            return Err(OsError::from_code(status));
        }
    }
    Ok(())
}

/// `Crypto_GetRandomBytes(count)`: `count` (0 a 4096) bytes aleatórios.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Crypto_GetRandomBytes(n: i64) -> i64 {
    if !(0..=4096).contains(&n) {
        let msg = allocate_str("Invalid argument: count must be a positive int less than or equal to 4096.");
        with_roots(&[msg], || dartforge_exception_throw(msg, 3));
        return 0;
    }
    let mut b = vec![0u8; n as usize];
    match system_random_bytes(&mut b) {
        Ok(()) => dart_bytes(b),
        Err(e) => {
            let error = e.to_dart();
            with_roots(&[error], || dartforge_exception_throw(error, 3));
            0
        }
    }
}

// ---------------------------------------------------------------------------
// O preparo do embedder.

/// O que o embedder da VM faz antes do `main` (`DartUtils::SetupIOLibrary`
/// e `PrepareCoreLibrary`): com `dart:io` no programa, o script
/// (`Platform.script`, o `argv[0]` do executável) e o gancho de `Uri.base`
/// (o diretório corrente). Sem `dart:io`, nada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_preparar_embedder() {
    let Some(iniciar) = ajudante("_dartforgeIniciarIo") else {
        return;
    };
    prepare_process_signals();
    // SAFETY: registrada pela biblioteca `dart:io` da sobreposição com a
    // assinatura (`String`) → `Object`.
    let iniciar = { let dart_target: usize = iniciar; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
    // O script: o que o hospedeiro definiu (o `.dart` que o JIT roda, como o
    // `dart run`), senão o `argv[0]` do executável (o AOT, como a VM).
    let defined = program_script().lock().unwrap_or_else(|e| e.into_inner()).clone();
    let script = defined.unwrap_or_else(|| std::env::args_os().next().map(|a| path_bytes_of(std::path::Path::new(&a))).unwrap_or_default());
    let script = dart_string_from_bytes(&script);
    let hook = with_roots(&[script], || iniciar(script));
    if dartforge_exception_pending() != 0 {
        return;
    }
    if let Some(define) = ajudante("_dartforgeDefinirUriBase") {
        // SAFETY: registrada pela biblioteca `dart:core` da sobreposição com
        // a assinatura (`Uri Function()`) → `void`.
        let define = { let dart_target: usize = define; move |a0: i64| { dart_v1(dart_target, a0) } };
        with_roots(&[hook], || define(hook));
    }
}

/// O `Platform::Initialize` da VM: escrever num pipe ou soquete fechado dá
/// o erro `EPIPE` em vez de matar o processo (SIGPIPE ignorado).
fn prepare_process_signals() {
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn signal(sig_num: i32, handler: usize) -> usize;
        }
        // SAFETY: SIGPIPE = 13, SIG_IGN = 1.
        unsafe { signal(13, 1) };
    }
}

// ---------------------------------------------------------------------------
// `Platform`.

/// A versão do SDK Dart que o programa compilou (`Platform.version`),
/// gravada pela entrada do programa.
static SDK_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Registra a versão do SDK (o arquivo `version` dele).
///
/// # Safety
/// `text` aponta para `n` bytes UTF-8 legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_versao_do_sdk(text: *const u8, n: i64) {
    // SAFETY: constante do módulo com `n` bytes.
    let b = unsafe { std::slice::from_raw_parts(text, n as usize) };
    let _ = SDK_VERSION.set(String::from_utf8_lossy(b).into_owned());
}

/// O nome do sistema como o Dart o informa (`Platform.operatingSystem`).
const SYSTEM_NAME: &str = if cfg!(target_os = "linux") {
    "linux"
} else if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(target_os = "windows") {
    "windows"
} else if cfg!(target_os = "android") {
    "android"
} else if cfg!(target_os = "ios") {
    "ios"
} else if cfg!(target_os = "fuchsia") {
    "fuchsia"
} else {
    "unknown"
};

/// A arquitetura como a VM a escreve em `Platform.version` (`x64`,
/// `arm64`…).
const ARCHITECTURE_NAME: &str = if cfg!(target_arch = "x86_64") {
    "x64"
} else if cfg!(target_arch = "aarch64") {
    "arm64"
} else if cfg!(target_arch = "x86") {
    "ia32"
} else if cfg!(target_arch = "arm") {
    "arm"
} else if cfg!(target_arch = "riscv64") {
    "riscv64"
} else {
    "unknown"
};

#[cfg(unix)]
fn processor_count() -> i64 {
    unsafe extern "C" {
        fn sysconf(name_str: i32) -> std::ffi::c_long;
    }
    #[cfg(target_os = "linux")]
    const SC_NPROCESSORS_ONLN: i32 = 84;
    #[cfg(not(target_os = "linux"))]
    const SC_NPROCESSORS_ONLN: i32 = 58;
    // SAFETY: consulta sem efeitos.
    unsafe { sysconf(SC_NPROCESSORS_ONLN) as i64 }
}
#[cfg(windows)]
fn processor_count() -> i64 {
    #[repr(C)]
    struct SystemInfo {
        architecture: u16,
        reserved: u16,
        page: u32,
        min: *mut std::ffi::c_void,
        maximum: *mut std::ffi::c_void,
        mask: usize,
        processors: u32,
        tipo: u32,
        granularity: u32,
        level: u16,
        revision: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemInfo(i: *mut SystemInfo);
    }
    // SAFETY: a estrutura é só dados; a função a preenche.
    let mut i: SystemInfo = unsafe { std::mem::zeroed() };
    unsafe { GetSystemInfo(&mut i) };
    i64::from(i.processors)
}

/// `Platform::OperatingSystemVersion`: `uname` no Linux ("sysname release
/// version"), o texto do `NSProcessInfo` no macOS ("Version 14.5 (Build
/// 23F79)"), e o do registro no Windows ("\"Windows 10 Pro\" 10.0 (Build
/// 19045)").
#[cfg(all(unix, not(target_os = "macos")))]
fn system_version() -> IoResult<String> {
    #[repr(C)]
    struct Utsname {
        fields: [[u8; 65]; 6],
    }
    unsafe extern "C" {
        fn uname(u: *mut Utsname) -> i32;
    }
    let mut u = Utsname { fields: [[0; 65]; 6] };
    // SAFETY: `u` tem o tamanho da `struct utsname` do Linux.
    if unsafe { uname(&mut u) } != 0 {
        return Err(OsError::de(&std::io::Error::last_os_error()));
    }
    let field = |i: usize| {
        let c = &u.fields[i];
        String::from_utf8_lossy(&c[..c.iter().position(|&b| b == 0).unwrap_or(c.len())]).into_owned()
    };
    Ok(format!("{} {} {}", field(0), field(2), field(3)))
}
#[cfg(target_os = "macos")]
fn system_version() -> IoResult<String> {
    fn sysctl_string(name_str: &std::ffi::CStr) -> IoResult<String> {
        unsafe extern "C" {
            fn sysctlbyname(name_str: *const std::ffi::c_char, v: *mut std::ffi::c_void, n: *mut usize, new: *mut std::ffi::c_void, nn: usize) -> i32;
        }
        let mut n = 0usize;
        // SAFETY: primeiro o tamanho, depois o valor num buffer desse tamanho.
        if unsafe { sysctlbyname(name_str.as_ptr(), std::ptr::null_mut(), &mut n, std::ptr::null_mut(), 0) } != 0 {
            return Err(OsError::de(&std::io::Error::last_os_error()));
        }
        let mut b = vec![0u8; n];
        if unsafe { sysctlbyname(name_str.as_ptr(), b.as_mut_ptr().cast(), &mut n, std::ptr::null_mut(), 0) } != 0 {
            return Err(OsError::de(&std::io::Error::last_os_error()));
        }
        b.truncate(b.iter().position(|&c| c == 0).unwrap_or(n));
        Ok(String::from_utf8_lossy(&b).into_owned())
    }
    Ok(format!("Version {} (Build {})", sysctl_string(c"kern.osproductversion")?, sysctl_string(c"kern.osversion")?))
}
#[cfg(windows)]
fn system_version() -> IoResult<String> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(
            key: isize,
            sub: *const u16,
            value: *const u16,
            flags: u32,
            tipo: *mut u32,
            data: *mut std::ffi::c_void,
            n: *mut u32,
        ) -> i32;
    }
    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as i32 as isize;
    const RRF_RT_REG_SZ: u32 = 0x2;
    const RRF_RT_REG_DWORD: u32 = 0x10;
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let sub = wide("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
    let text = |name_str: &str| -> IoResult<String> {
        let mut b = [0u16; 256];
        let mut n = std::mem::size_of_val(&b) as u32;
        // SAFETY: o buffer tem `n` bytes.
        let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, sub.as_ptr(), wide(name_str).as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), b.as_mut_ptr().cast(), &mut n) };
        if r != 0 {
            return Err(OsError::from_code(r));
        }
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        Ok(String::from_utf16_lossy(&b[..end]))
    };
    let word = |name_str: &str| -> Option<u32> {
        let mut v = 0u32;
        let mut n = 4u32;
        // SAFETY: `v` tem 4 bytes.
        let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, sub.as_ptr(), wide(name_str).as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut v as *mut u32).cast(), &mut n) };
        (r == 0).then_some(v)
    };
    let name_str = text("ProductName")?;
    let number = match word("CurrentMajorVersionNumber") {
        Some(larger) => format!("{larger}.{}", word("CurrentMinorVersionNumber").ok_or_else(|| OsError::from_code(2))?),
        None => text("CurrentVersion")?,
    };
    Ok(format!("\"{name_str}\" {number} (Build {})", text("CurrentBuild")?))
}

/// `Platform::LocalHostname`.
#[cfg(unix)]
fn host_name() -> IoResult<String> {
    unsafe extern "C" {
        fn gethostname(name_str: *mut std::ffi::c_char, n: usize) -> i32;
    }
    let mut b = [0u8; 256];
    // SAFETY: o buffer tem 256 bytes.
    if unsafe { gethostname(b.as_mut_ptr().cast(), b.len()) } != 0 {
        return Err(OsError::de(&std::io::Error::last_os_error()));
    }
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    Ok(String::from_utf8_lossy(&b[..end]).into_owned())
}
#[cfg(windows)]
fn host_name() -> IoResult<String> {
    #[link(name = "ws2_32")]
    unsafe extern "system" {
        fn WSAStartup(version: u16, data: *mut u8) -> i32;
        fn GetHostNameW(name_str: *mut u16, n: i32) -> i32;
        fn WSAGetLastError() -> i32;
    }
    let mut data = [0u8; 512];
    // SAFETY: `WSADATA` cabe em 512 bytes; a inicialização é contada.
    if unsafe { WSAStartup(0x0202, data.as_mut_ptr()) } != 0 {
        return Err(OsError::from_code(unsafe { WSAGetLastError() }));
    }
    let mut b = [0u16; 256];
    // SAFETY: o buffer tem 256 unidades.
    if unsafe { GetHostNameW(b.as_mut_ptr(), b.len() as i32) } != 0 {
        return Err(OsError::from_code(unsafe { WSAGetLastError() }));
    }
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    Ok(String::from_utf16_lossy(&b[..end]))
}

/// `Platform::LocaleName`: `LANG` (ou `en_US`) no Linux; a língua preferida
/// (ou a localidade corrente) no macOS; `GetUserDefaultLocaleName` no
/// Windows.
#[cfg(all(unix, not(target_os = "macos")))]
fn locale_name() -> IoResult<String> {
    Ok(std::env::var("LANG").unwrap_or_else(|_| "en_US".to_string()))
}
#[cfg(target_os = "macos")]
fn locale_name() -> IoResult<String> {
    type Ref = *const std::ffi::c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFLocaleCopyPreferredLanguages() -> Ref;
        fn CFArrayGetCount(a: Ref) -> isize;
        fn CFArrayGetValueAtIndex(a: Ref, i: isize) -> Ref;
        fn CFLocaleCopyCurrent() -> Ref;
        fn CFLocaleGetIdentifier(l: Ref) -> Ref;
        fn CFStringGetLength(s: Ref) -> isize;
        fn CFStringGetMaximumSizeForEncoding(n: isize, encoding: u32) -> isize;
        fn CFStringGetCString(s: Ref, b: *mut std::ffi::c_char, n: isize, encoding: u32) -> u8;
        fn CFRelease(r: Ref);
    }
    const UTF8: u32 = 0x0800_0100;
    fn text(s: Ref) -> Option<String> {
        // SAFETY: `s` é uma CFString viva; o buffer tem o tamanho máximo.
        unsafe {
            let n = CFStringGetMaximumSizeForEncoding(CFStringGetLength(s), UTF8) + 1;
            let mut b = vec![0u8; n as usize];
            if CFStringGetCString(s, b.as_mut_ptr().cast(), n, UTF8) == 0 {
                return None;
            }
            b.truncate(b.iter().position(|&c| c == 0).unwrap_or(b.len()));
            Some(String::from_utf8_lossy(&b).into_owned())
        }
    }
    // SAFETY: objetos CoreFoundation criados e soltos aqui.
    unsafe {
        let languages = CFLocaleCopyPreferredLanguages();
        let preferred = if CFArrayGetCount(languages) >= 1 { text(CFArrayGetValueAtIndex(languages, 0)) } else { None };
        CFRelease(languages);
        if let Some(l) = preferred {
            return Ok(l);
        }
        let local = CFLocaleCopyCurrent();
        let r = text(CFLocaleGetIdentifier(local));
        CFRelease(local);
        r.ok_or_else(|| OsError::from_code(os_code::INVALID))
    }
}
#[cfg(windows)]
fn locale_name() -> IoResult<String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultLocaleName(b: *mut u16, n: i32) -> i32;
    }
    let mut b = [0u16; 85];
    // SAFETY: o buffer tem `LOCALE_NAME_MAX_LENGTH` unidades.
    let n = unsafe { GetUserDefaultLocaleName(b.as_mut_ptr(), b.len() as i32) };
    if n == 0 {
        return Err(OsError::de(&std::io::Error::last_os_error()));
    }
    Ok(String::from_utf16_lossy(&b[..(n as usize).saturating_sub(1)]))
}

/// As variáveis de ambiente como `NOME=value` (as que não são UTF-8 válido
/// ficam de fora, como na VM; no Windows, também as que começam com `=`).
fn environment_variables() -> Vec<String> {
    let mut out = Vec::new();
    for (k, v) in std::env::vars_os() {
        #[cfg(windows)]
        if k.to_string_lossy().starts_with('=') {
            continue;
        }
        if let (Some(k), Some(v)) = (k.to_str(), v.to_str()) {
            out.push(format!("{k}={v}"));
        }
    }
    out
}

/// Uma `_List` de tamanho fixo com as strings; `tipada`: `List<String>`
/// (o `Dart_NewListOfTypeFilled` da VM), senão `List<dynamic>`.
fn dart_string_list(textos: &[String], tipada: bool) -> i64 {
    let list = dart_fixed_list(&vec![0; textos.len()]);
    with_roots(&[list], || {
        for (i, t) in textos.iter().enumerate() {
            let s = allocate_str(t);
            HEAP.with(|h| h.borrow_mut().list_set(list, i, crate::heap::Value::Ref(s)));
        }
        if tipada {
            let cid = HEAP.with(|h| h.borrow().class(list));
            if let Some(tipo) = string_list_type(Some(i64::from(cid))) {
                HEAP.with(|h| h.borrow_mut().set_metadata(list, tipo + 1));
            }
        }
    });
    list
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_NumberOfProcessors() -> i64 {
    processor_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_PathSeparator() -> i64 {
    allocate_str(PATH_SEPARATOR)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_OperatingSystem() -> i64 {
    allocate_str(SYSTEM_NAME)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_OperatingSystemVersion() -> i64 {
    dart_or_error(system_version(), |v| allocate_str(&v))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_LocalHostname() -> i64 {
    dart_or_error(host_name(), |v| allocate_str(&v))
}

/// `Platform_ExecutableName`: o `argv[0]` do processo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_ExecutableName() -> i64 {
    match std::env::args_os().next() {
        Some(a) => dart_string_from_bytes(&path_bytes_of(std::path::Path::new(&a))),
        None => 0,
    }
}

/// `Platform_ResolvedExecutableName`: o caminho absoluto e canônico do
/// executável.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_ResolvedExecutableName() -> i64 {
    match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(p) => {
            #[cfg(windows)]
            let p = {
                let s = p.to_string_lossy().into_owned();
                std::path::PathBuf::from(s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s))
            };
            dart_string_from_bytes(&path_bytes_of(&p))
        }
        Err(_) => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_Environment() -> i64 {
    dart_string_list(&environment_variables(), false)
}

/// `Platform_ExecutableArguments`: as opções da VM antes do script — um
/// executável AOT não tem nenhuma.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_ExecutableArguments() -> i64 {
    dart_string_list(&[], true)
}

/// Os argumentos do `main`: os da linha de comando depois do executável
/// (AOT), ou os que o executor do JIT definiu ([`set_main_arguments`]).
fn main_arguments() -> &'static std::sync::Mutex<Option<Vec<String>>> {
    static A: std::sync::Mutex<Option<Vec<String>>> = std::sync::Mutex::new(None);
    &A
}

/// O `Platform.script` quando o processo não é o próprio programa (o JIT):
/// os bytes do caminho do `.dart`.
fn program_script() -> &'static std::sync::Mutex<Option<Vec<u8>>> {
    static S: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);
    &S
}

/// Define o `Platform.script` do programa (antes do `main`).
pub fn set_program_script(file_path: Vec<u8>) {
    *program_script().lock().unwrap_or_else(|e| e.into_inner()) = Some(file_path);
}

/// [`set_program_script`] pela fronteira C (o runtime da biblioteca
/// do SDK da fonte): o caminho em UTF-8.
///
/// # Safety
/// `data` aponta `size` bytes legíveis (ou `size` é 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_definir_script_do_programa(data: *const u8, size: usize) {
    let bytes = if size == 0 {
        Vec::new()
    } else {
        // SAFETY: garantido pelo chamador.
        unsafe { std::slice::from_raw_parts(data, size) }.to_vec()
    };
    set_program_script(bytes);
}

/// Os argumentos que o `main(List<String> args)` recebe quando o processo
/// não é o próprio programa (o executor do JIT).
pub fn set_main_arguments(arguments: Vec<String>) {
    *main_arguments().lock().unwrap_or_else(|e| e.into_inner()) = Some(arguments);
}

/// [`set_main_arguments`] pela fronteira C, para o executor do JIT
/// alcançar o runtime da biblioteca do SDK da fonte: os argumentos em UTF-8,
/// cada um terminado em NUL.
///
/// # Safety
/// `data` aponta `size` bytes legíveis (ou `size` é 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_definir_argumentos_do_main(data: *const u8, size: usize) {
    let bytes = if size == 0 {
        &[][..]
    } else {
        // SAFETY: garantido pelo chamador.
        unsafe { std::slice::from_raw_parts(data, size) }
    };
    let mut arguments: Vec<String> = bytes.split(|&b| b == 0).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
    // O último NUL deixa um pedaço vazio que não é argumento.
    arguments.pop();
    set_main_arguments(arguments);
}

/// O `args` do `main`: uma `List<String>` de tamanho fixo, como a do
/// embedder da VM (`Dart_NewListOfType`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_argumentos_do_main() -> i64 {
    let arguments = main_arguments()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_else(|| std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect());
    dart_string_list(&arguments, true)
}

/// `Platform_GetVersion`: a versão do SDK, o canal e o alvo, na forma do
/// `Dart_VersionString` da VM (sem a data de compilação da VM, que não
/// existe aqui).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_GetVersion() -> i64 {
    let version = SDK_VERSION.get().map_or("unknown", String::as_str);
    // A versão completa, como a VM a escreve (o compilador a perguntou ao
    // `dart --version`, com o canal e a data): só o sistema falta.
    if version.contains(" (") {
        return allocate_str(&format!("{version} on \"{SYSTEM_NAME}_{ARCHITECTURE_NAME}\""));
    }
    let chan = if version.contains("-edge") {
        "edge"
    } else if version.contains(".dev") || version.contains("-dev") {
        "dev"
    } else if version.contains(".beta") || version.contains("-beta") {
        "beta"
    } else {
        "stable"
    };
    allocate_str(&format!("{version} ({chan}) on \"{SYSTEM_NAME}_{ARCHITECTURE_NAME}\""))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_LocaleName() -> i64 {
    dart_or_error(locale_name(), |v| allocate_str(&v))
}

// ---------------------------------------------------------------------------
// O terminal (`Stdin`/`Stdout`).

/// Os modos de `Stdin`: eco, eco da nova linha e modo de linha.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TerminalMode {
    Echo,
    EchoNewline,
    Line,
}

/// A `struct termios` como bytes (o layout muda entre Linux e macOS; só o
/// `c_lflag` é lido e gravado).
#[cfg(unix)]
#[repr(C, align(8))]
struct Termios([u8; 128]);

#[cfg(unix)]
impl Termios {
    #[cfg(target_os = "linux")]
    fn lflag(&self) -> u64 {
        u64::from(u32::from_ne_bytes(self.0[12..16].try_into().expect("4 bytes")))
    }
    #[cfg(target_os = "linux")]
    fn write_lflag(&mut self, v: u64) {
        self.0[12..16].copy_from_slice(&(v as u32).to_ne_bytes());
    }
    #[cfg(not(target_os = "linux"))]
    fn lflag(&self) -> u64 {
        u64::from_ne_bytes(self.0[24..32].try_into().expect("8 bytes"))
    }
    #[cfg(not(target_os = "linux"))]
    fn write_lflag(&mut self, v: u64) {
        self.0[24..32].copy_from_slice(&v.to_ne_bytes());
    }
}

#[cfg(unix)]
fn mode_bit(m: TerminalMode) -> u64 {
    #[cfg(target_os = "linux")]
    let (echo, echo_nl, line) = (0o10, 0o100, 0o2);
    #[cfg(not(target_os = "linux"))]
    let (echo, echo_nl, line) = (0x8, 0x10, 0x100);
    match m {
        TerminalMode::Echo => echo,
        TerminalMode::EchoNewline => echo_nl,
        TerminalMode::Line => line,
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn tcgetattr(fd: i32, t: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, action: i32, t: *const Termios) -> i32;
    fn isatty(fd: i32) -> i32;
    fn ioctl(fd: i32, request: std::ffi::c_ulong, ...) -> i32;
}

#[cfg(unix)]
fn terminal_attributes(fd: i64) -> IoResult<Termios> {
    let mut t = Termios([0; 128]);
    // SAFETY: `t` é maior que a `struct termios` de qualquer plataforma.
    if unsafe { tcgetattr(fd as i32, &mut t) } != 0 {
        return Err(OsError::de(&std::io::Error::last_os_error()));
    }
    Ok(t)
}

#[cfg(unix)]
fn terminal_mode(fd: i64, m: TerminalMode) -> IoResult<bool> {
    Ok(terminal_attributes(fd)?.lflag() & mode_bit(m) != 0)
}

#[cfg(unix)]
fn change_terminal_mode(fd: i64, m: TerminalMode, enabled_flag: bool) -> IoResult<()> {
    let mut t = terminal_attributes(fd)?;
    let bit = mode_bit(m);
    let f = t.lflag();
    t.write_lflag(if enabled_flag { f | bit } else { f & !bit });
    // SAFETY: `t` veio de `tcgetattr` (TCSANOW = 0).
    if unsafe { tcsetattr(fd as i32, 0, &t) } != 0 {
        return Err(OsError::de(&std::io::Error::last_os_error()));
    }
    Ok(())
}

/// `TermIsKnownToSupportAnsi` e `isatty`, como a VM.
#[cfg(unix)]
fn supports_ansi(fd: i64) -> bool {
    let term = std::env::var("TERM").unwrap_or_default();
    // SAFETY: consulta sem efeitos.
    (unsafe { isatty(fd as i32) } != 0) && ["xterm", "screen", "rxvt"].iter().any(|t| term.contains(t))
}

#[cfg(unix)]
fn terminal_size(fd: i64) -> IoResult<(i64, i64)> {
    #[repr(C)]
    struct Winsize {
        line_list: u16,
        columns: u16,
        x: u16,
        y: u16,
    }
    #[cfg(target_os = "linux")]
    const TIOCGWINSZ: std::ffi::c_ulong = 0x5413;
    #[cfg(not(target_os = "linux"))]
    const TIOCGWINSZ: std::ffi::c_ulong = 0x4008_7468;
    let mut w = Winsize { line_list: 0, columns: 0, x: 0, y: 0 };
    // SAFETY: `w` é a `struct winsize`.
    let r = unsafe { ioctl(fd as i32, TIOCGWINSZ, &mut w as *mut Winsize) };
    if r == 0 && (w.columns != 0 || w.line_list != 0) {
        return Ok((i64::from(w.columns), i64::from(w.line_list)));
    }
    Err(OsError::de(&std::io::Error::last_os_error()))
}

#[cfg(unix)]
fn read_stdin_byte(fd: i64) -> IoResult<i64> {
    let f = std::mem::ManuallyDrop::new(file_from_descriptor(fd));
    let mut b = [0u8; 1];
    loop {
        use std::io::Read;
        match (&*f).read(&mut b) {
            Ok(0) => return Ok(-1),
            Ok(_) => return Ok(i64::from(b[0])),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(OsError::de(&e)),
        }
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleMode(h: *mut std::ffi::c_void, mode: *mut u32) -> i32;
    fn SetConsoleMode(h: *mut std::ffi::c_void, mode: u32) -> i32;
}

#[cfg(windows)]
fn default_handle_of(fd: i64) -> *mut std::ffi::c_void {
    default_descriptor(fd) as usize as *mut std::ffi::c_void
}

#[cfg(windows)]
fn console_mode(fd: i64) -> IoResult<u32> {
    let mut m = 0u32;
    // SAFETY: consulta o modo de um handle do processo.
    if unsafe { GetConsoleMode(default_handle_of(fd), &mut m) } == 0 {
        return Err(OsError::de(&std::io::Error::last_os_error()));
    }
    Ok(m)
}

#[cfg(windows)]
fn terminal_mode(_fd: i64, m: TerminalMode) -> IoResult<bool> {
    match m {
        TerminalMode::Echo => Ok(console_mode(0)? & 0x4 != 0), // ENABLE_ECHO_INPUT
        TerminalMode::Line => Ok(console_mode(0)? & 0x2 != 0), // ENABLE_LINE_INPUT
        TerminalMode::EchoNewline => Ok(false),
    }
}

#[cfg(windows)]
fn change_terminal_mode(_fd: i64, m: TerminalMode, enabled_flag: bool) -> IoResult<()> {
    let bit = match m {
        TerminalMode::Echo => 0x4,
        TerminalMode::Line => 0x2,
        TerminalMode::EchoNewline if enabled_flag => return Err(OsError::from_code(775 /* ERROR_NOT_CAPABLE */)),
        TerminalMode::EchoNewline => return Ok(()),
    };
    let current = console_mode(0)?;
    let new = if enabled_flag { current | bit } else { current & !bit };
    // SAFETY: muda o modo de um handle do processo.
    if unsafe { SetConsoleMode(default_handle_of(0), new) } == 0 {
        return Err(OsError::de(&std::io::Error::last_os_error()));
    }
    Ok(())
}

/// `ENABLE_VIRTUAL_TERMINAL_INPUT` (entrada) / `_PROCESSING` (saída).
#[cfg(windows)]
fn supports_ansi(fd: i64) -> bool {
    let bit = if fd == 0 { 0x200 } else { 0x4 };
    console_mode(fd).is_ok_and(|m| m & bit != 0)
}

#[cfg(windows)]
fn terminal_size(fd: i64) -> IoResult<(i64, i64)> {
    #[repr(C)]
    struct Info {
        size: [i16; 2],
        cursor: [i16; 2],
        attributes: u16,
        window: [i16; 4],
        maximum: [i16; 2],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleScreenBufferInfo(h: *mut std::ffi::c_void, i: *mut Info) -> i32;
    }
    // SAFETY: a estrutura é só dados; a função a preenche.
    let mut i: Info = unsafe { std::mem::zeroed() };
    if unsafe { GetConsoleScreenBufferInfo(default_handle_of(if fd == 1 { 1 } else { 2 }), &mut i) } == 0 {
        return Err(OsError::de(&std::io::Error::last_os_error()));
    }
    let [left, top, right, base] = i.window;
    Ok((i64::from(right - left + 1), i64::from(base - top + 1)))
}

#[cfg(windows)]
fn read_stdin_byte(_fd: i64) -> IoResult<i64> {
    let f = std::mem::ManuallyDrop::new(file_from_descriptor(default_descriptor(0)));
    let mut b = [0u8; 1];
    use std::io::Read;
    match (&*f).read(&mut b) {
        Ok(0) => Ok(-1),
        Ok(_) => Ok(i64::from(b[0])),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(-1),
        Err(e) => Err(OsError::de(&e)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_ReadByte(fd: i64) -> i64 {
    dart_or_error(read_stdin_byte(fd), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_GetEchoMode(fd: i64) -> i64 {
    dart_or_error(terminal_mode(fd, TerminalMode::Echo), dart_bool)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_SetEchoMode(fd: i64, enabled_flag: u8) -> i64 {
    dart_true_or_error(change_terminal_mode(fd, TerminalMode::Echo, enabled_flag != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_GetEchoNewlineMode(fd: i64) -> i64 {
    dart_or_error(terminal_mode(fd, TerminalMode::EchoNewline), dart_bool)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_SetEchoNewlineMode(fd: i64, enabled_flag: u8) -> i64 {
    dart_true_or_error(change_terminal_mode(fd, TerminalMode::EchoNewline, enabled_flag != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_GetLineMode(fd: i64) -> i64 {
    dart_or_error(terminal_mode(fd, TerminalMode::Line), dart_bool)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_SetLineMode(fd: i64, enabled_flag: u8) -> i64 {
    dart_true_or_error(change_terminal_mode(fd, TerminalMode::Line, enabled_flag != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_AnsiSupported(fd: i64) -> i64 {
    dart_bool(supports_ansi(fd))
}

/// `Stdout_GetTerminalSize`: `[columns, line_list]` ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdout_GetTerminalSize(fd: i64) -> i64 {
    dart_or_error(terminal_size(fd), |(c, l)| dart_fixed_list(&[dart_int(c), dart_int(l)]))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdout_AnsiSupported(fd: i64) -> i64 {
    dart_bool(supports_ansi(fd))
}

// ---------------------------------------------------------------------------
// O processo corrente (`exit`, `exitCode`, `sleep`, `pid`).

/// O código de saída global (`Process::GlobalExitCode`), que o fim normal
/// do programa devolve.
static EXIT_CODE: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// O código com que o processo termina quando o programa acaba sem erro.
fn global_exit_code() -> i32 {
    EXIT_CODE.load(std::sync::atomic::Ordering::Relaxed) as i32
}

/// `Process_Exit`: termina o processo já (a VM usa `_exit`, sem os
/// destrutores globais; a saída padrão do `std` é esvaziada antes).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_Exit(status: i64) {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    std::process::exit(status as i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_SetExitCode(status: i64) {
    EXIT_CODE.store(status, std::sync::atomic::Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_GetExitCode() -> i64 {
    EXIT_CODE.load(std::sync::atomic::Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_Sleep(ms: i64) {
    std::thread::sleep(std::time::Duration::from_millis(ms.max(0) as u64));
}

/// `Process_Pid(process)`: o do processo corrente (com `null`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_Pid(process: i64) -> i64 {
    if process == 0 {
        return i64::from(std::process::id());
    }
    native_field(process)
}

// ---------------------------------------------------------------------------
// A codificação do sistema (`SystemEncodingToString`/`StringToSystemEncoding`,
// o `StringUtils::ConsoleStringToUtf8` da VM): UTF-8 no Unix; a página de
// código do console (`GetConsoleCP`) no Windows.

#[cfg(unix)]
fn system_to_utf8(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}
#[cfg(unix)]
fn utf8_to_system(s: &[u8]) -> Vec<u8> {
    s.to_vec()
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleCP() -> u32;
    fn MultiByteToWideChar(cp: u32, flags: u32, s: *const u8, n: i32, w: *mut u16, nw: i32) -> i32;
    fn WideCharToMultiByte(cp: u32, flags: u32, w: *const u16, nw: i32, s: *mut u8, n: i32, pattern: *const u8, used_it: *mut i32) -> i32;
}
#[cfg(windows)]
fn system_to_utf8(b: &[u8]) -> String {
    if b.is_empty() {
        return String::new();
    }
    // SAFETY: tamanhos consultados antes; os buffers têm o tamanho pedido.
    unsafe {
        let cp = GetConsoleCP();
        let n = MultiByteToWideChar(cp, 0, b.as_ptr(), b.len() as i32, std::ptr::null_mut(), 0);
        let mut w = vec![0u16; n.max(0) as usize];
        MultiByteToWideChar(cp, 0, b.as_ptr(), b.len() as i32, w.as_mut_ptr(), n);
        String::from_utf16_lossy(&w)
    }
}
#[cfg(windows)]
fn utf8_to_system(s: &[u8]) -> Vec<u8> {
    let w: Vec<u16> = String::from_utf8_lossy(s).encode_utf16().collect();
    if w.is_empty() {
        return Vec::new();
    }
    // SAFETY: tamanhos consultados antes; os buffers têm o tamanho pedido.
    unsafe {
        let cp = GetConsoleCP();
        let n = WideCharToMultiByte(cp, 0, w.as_ptr(), w.len() as i32, std::ptr::null_mut(), 0, std::ptr::null(), std::ptr::null_mut());
        let mut b = vec![0u8; n.max(0) as usize];
        WideCharToMultiByte(cp, 0, w.as_ptr(), w.len() as i32, b.as_mut_ptr(), n, std::ptr::null(), std::ptr::null_mut());
        b
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SystemEncodingToString(bytes: i64) -> i64 {
    let b = typed_list_bytes(bytes).unwrap_or_else(|| {
        HEAP.with(|h| {
            let h = h.borrow();
            bytes_from_value_list(&h, bytes)
        })
    });
    allocate_str(&system_to_utf8(&b))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StringToSystemEncoding(text: i64) -> i64 {
    dart_bytes(utf8_to_system(&utf8_of_string(text)))
}
