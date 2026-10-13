// Native runtime: the VM's `dart:io` — platform, terminal and process
// (`runtime/bin/platform*.cc`, `stdio*.cc`, `crypto*.cc` and the simple natives
// of `process.cc`): `Platform`, `Stdin`/`Stdout`, `exit`, `exitCode`, `sleep`,
// `pid` and the system's random bytes.

// ---------------------------------------------------------------------------
// System random bytes (`Crypto::GetRandomBytes`).

/// Fills `dest` with bytes from the system's secure generator: `getentropy` on
/// Unix (in blocks of 256, the maximum per call) and `BCryptGenRandom` on Windows.
#[cfg(unix)]
fn system_random_bytes(dest: &mut [u8]) -> IoResult<()> {
    unsafe extern "C" {
        fn getentropy(buf: *mut std::ffi::c_void, n: usize) -> i32;
    }
    for block in dest.chunks_mut(256) {
        // SAFETY: `block` is a writable chunk of up to 256 bytes.
        if unsafe { getentropy(block.as_mut_ptr().cast(), block.len()) } != 0 {
            return Err(OsError::of(&std::io::Error::last_os_error()));
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
        // SAFETY: `block` is writable with the size passed.
        let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), block.as_mut_ptr(), block.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
        if status != 0 {
            return Err(OsError::from_code(status));
        }
    }
    Ok(())
}

/// `Crypto_GetRandomBytes(count)`: `count` (0 to 4096) random bytes.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Crypto_GetRandomBytes(n: i64) -> i64 {
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
// The embedder setup.

/// What the VM's embedder does before `main` (`DartUtils::SetupIOLibrary` and
/// `PrepareCoreLibrary`): with `dart:io` in the program, the script
/// (`Platform.script`, the executable's `argv[0]`) and the `Uri.base` hook (the
/// current directory). Without `dart:io`, nothing.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_prepare_embedder() {
    let Some(start) = helper("_dartforgeIniciarIo") else {
        return;
    };
    prepare_process_signals();
    // SAFETY: registered by the overlay's `dart:io` library with the signature
    // (`String`) → `Object`.
    let start = { let dart_target: usize = start; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
    // The script: what the host set (the `.dart` the JIT runs, like `dart run`),
    // otherwise the executable's `argv[0]` (the AOT, like the VM).
    let defined = program_script().lock().unwrap_or_else(|e| e.into_inner()).clone();
    let script = defined.unwrap_or_else(|| std::env::args_os().next().map(|a| path_bytes_of(std::path::Path::new(&a))).unwrap_or_default());
    let script = dart_string_from_bytes(&script);
    let hook = with_roots(&[script], || start(script));
    if dartforge_exception_pending() != 0 {
        return;
    }
    if let Some(define) = helper("_dartforgeDefinirUriBase") {
        // SAFETY: registered by the overlay's `dart:core` library with the
        // signature (`Uri Function()`) → `void`.
        let define = { let dart_target: usize = define; move |a0: i64| { dart_v1(dart_target, a0) } };
        with_roots(&[hook], || define(hook));
    }
}

/// The VM's `Platform::Initialize`: writing to a closed pipe or socket gives the
/// `EPIPE` error instead of killing the process (SIGPIPE ignored).
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

/// The version of the Dart SDK the program was compiled with (`Platform.version`),
/// stored by the program's entry.
static SDK_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Registers the SDK version (its `version` file).
///
/// # Safety
/// `text` points to `n` readable UTF-8 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_sdk_version(text: *const u8, n: i64) {
    // SAFETY: a module constant with `n` bytes.
    let b = unsafe { std::slice::from_raw_parts(text, n as usize) };
    let _ = SDK_VERSION.set(String::from_utf8_lossy(b).into_owned());
}

/// The system name as Dart reports it (`Platform.operatingSystem`).
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

/// The architecture as the VM writes it in `Platform.version` (`x64`, `arm64`…).
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
    // SAFETY: a query without effects.
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
        ty: u32,
        granularity: u32,
        level: u16,
        revision: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemInfo(i: *mut SystemInfo);
    }
    // SAFETY: the structure is plain data; the function fills it.
    let mut i: SystemInfo = unsafe { std::mem::zeroed() };
    unsafe { GetSystemInfo(&mut i) };
    i64::from(i.processors)
}

/// `Platform::OperatingSystemVersion`: `uname` on Linux ("sysname release
/// version"), the `NSProcessInfo` text on macOS ("Version 14.5 (Build 23F79)"),
/// and the registry's on Windows ("\"Windows 10 Pro\" 10.0 (Build 19045)").
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
    // SAFETY: `u` has the size of Linux's `struct utsname`.
    if unsafe { uname(&mut u) } != 0 {
        return Err(OsError::of(&std::io::Error::last_os_error()));
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
        // SAFETY: first the size, then the value into a buffer of that size.
        if unsafe { sysctlbyname(name_str.as_ptr(), std::ptr::null_mut(), &mut n, std::ptr::null_mut(), 0) } != 0 {
            return Err(OsError::of(&std::io::Error::last_os_error()));
        }
        let mut b = vec![0u8; n];
        if unsafe { sysctlbyname(name_str.as_ptr(), b.as_mut_ptr().cast(), &mut n, std::ptr::null_mut(), 0) } != 0 {
            return Err(OsError::of(&std::io::Error::last_os_error()));
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
            ty: *mut u32,
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
        // SAFETY: the buffer has `n` bytes.
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
        // SAFETY: `v` has 4 bytes.
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
    // SAFETY: the buffer has 256 bytes.
    if unsafe { gethostname(b.as_mut_ptr().cast(), b.len()) } != 0 {
        return Err(OsError::of(&std::io::Error::last_os_error()));
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
    // SAFETY: `WSADATA` fits in 512 bytes; the initialization is counted.
    if unsafe { WSAStartup(0x0202, data.as_mut_ptr()) } != 0 {
        return Err(OsError::from_code(unsafe { WSAGetLastError() }));
    }
    let mut b = [0u16; 256];
    // SAFETY: the buffer has 256 units.
    if unsafe { GetHostNameW(b.as_mut_ptr(), b.len() as i32) } != 0 {
        return Err(OsError::from_code(unsafe { WSAGetLastError() }));
    }
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    Ok(String::from_utf16_lossy(&b[..end]))
}

/// `Platform::LocaleName`: `LANG` (or `en_US`) on Linux; the preferred language
/// (or the current locale) on macOS; `GetUserDefaultLocaleName` on Windows.
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
        // SAFETY: `s` is a live CFString; the buffer has the maximum size.
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
    // SAFETY: CoreFoundation objects created and released here.
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
    // SAFETY: the buffer has `LOCALE_NAME_MAX_LENGTH` units.
    let n = unsafe { GetUserDefaultLocaleName(b.as_mut_ptr(), b.len() as i32) };
    if n == 0 {
        return Err(OsError::of(&std::io::Error::last_os_error()));
    }
    Ok(String::from_utf16_lossy(&b[..(n as usize).saturating_sub(1)]))
}

/// The environment variables as `NAME=value` (those that are not valid UTF-8 are
/// left out, as in the VM; on Windows, also those starting with `=`).
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

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Platform_NumberOfProcessors() -> i64 {
    processor_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Platform_PathSeparator() -> i64 {
    allocate_str(PATH_SEPARATOR)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Platform_OperatingSystem() -> i64 {
    allocate_str(SYSTEM_NAME)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Platform_OperatingSystemVersion() -> i64 {
    dart_or_error(system_version(), |v| allocate_str(&v))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Platform_LocalHostname() -> i64 {
    dart_or_error(host_name(), |v| allocate_str(&v))
}

/// `Platform_ExecutableName`: the process's `argv[0]`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Platform_ExecutableName() -> i64 {
    match std::env::args_os().next() {
        Some(a) => dart_string_from_bytes(&path_bytes_of(std::path::Path::new(&a))),
        None => 0,
    }
}

/// `Platform_ResolvedExecutableName`: the absolute, canonical path of the
/// executable.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Platform_ResolvedExecutableName() -> i64 {
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
pub extern "C" fn dartforge_native_Platform_Environment() -> i64 {
    dart_string_list(&environment_variables(), false)
}

/// `Platform_ExecutableArguments`: the VM options before the script — an AOT
/// executable has none.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Platform_ExecutableArguments() -> i64 {
    dart_string_list(&[], true)
}

/// The arguments of `main`: those of the command line after the executable (AOT),
/// or those the JIT runner set ([`set_main_arguments`]).
fn main_arguments() -> &'static std::sync::Mutex<Option<Vec<String>>> {
    static A: std::sync::Mutex<Option<Vec<String>>> = std::sync::Mutex::new(None);
    &A
}

/// The `Platform.script` when the process is not the program itself (the JIT): the
/// bytes of the `.dart` path.
fn program_script() -> &'static std::sync::Mutex<Option<Vec<u8>>> {
    static S: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);
    &S
}

/// Sets the program's `Platform.script` (before `main`).
pub fn set_program_script(file_path: Vec<u8>) {
    *program_script().lock().unwrap_or_else(|e| e.into_inner()) = Some(file_path);
}

/// [`set_program_script`] across the C boundary (the runtime of the SDK-from-source
/// library): the path in UTF-8.
///
/// # Safety
/// `data` points to `size` readable bytes (or `size` is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_set_program_script(data: *const u8, size: usize) {
    let bytes = if size == 0 {
        Vec::new()
    } else {
        // SAFETY: guaranteed by the caller.
        unsafe { std::slice::from_raw_parts(data, size) }.to_vec()
    };
    set_program_script(bytes);
}

/// The arguments that `main(List<String> args)` receives when the process is not
/// the program itself (the JIT runner).
pub fn set_main_arguments(arguments: Vec<String>) {
    *main_arguments().lock().unwrap_or_else(|e| e.into_inner()) = Some(arguments);
}

/// [`set_main_arguments`] across the C boundary, for the JIT runner to reach the
/// runtime of the SDK-from-source library: the arguments in UTF-8, each one
/// NUL-terminated.
///
/// # Safety
/// `data` points to `size` readable bytes (or `size` is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_set_main_arguments(data: *const u8, size: usize) {
    let bytes = if size == 0 {
        &[][..]
    } else {
        // SAFETY: guaranteed by the caller.
        unsafe { std::slice::from_raw_parts(data, size) }
    };
    let mut arguments: Vec<String> = bytes.split(|&b| b == 0).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
    // The last NUL leaves an empty piece that is not an argument.
    arguments.pop();
    set_main_arguments(arguments);
}

/// The `args` of `main`: a fixed-length `List<String>`, like the VM embedder's
/// (`Dart_NewListOfType`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_main_arguments() -> i64 {
    let arguments = main_arguments()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_else(|| std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect());
    dart_string_list(&arguments, true)
}

/// `Platform_GetVersion`: the SDK version, the channel and the target, in the form
/// of the VM's `Dart_VersionString` (without the VM's build date, which does not
/// exist here).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Platform_GetVersion() -> i64 {
    let version = SDK_VERSION.get().map_or("unknown", String::as_str);
    // The full version, as the VM writes it (the compiler asked `dart --version`
    // for it, with the channel and the date): only the system is missing.
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
pub extern "C" fn dartforge_native_Platform_LocaleName() -> i64 {
    dart_or_error(locale_name(), |v| allocate_str(&v))
}

// ---------------------------------------------------------------------------
// The terminal (`Stdin`/`Stdout`).

/// The `Stdin` modes: echo, newline echo and line mode.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TerminalMode {
    Echo,
    EchoNewline,
    Line,
}

/// The `struct termios` as bytes (the layout changes between Linux and macOS; only
/// `c_lflag` is read and written).
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
    // SAFETY: `t` is larger than the `struct termios` of any platform.
    if unsafe { tcgetattr(fd as i32, &mut t) } != 0 {
        return Err(OsError::of(&std::io::Error::last_os_error()));
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
    // SAFETY: `t` came from `tcgetattr` (TCSANOW = 0).
    if unsafe { tcsetattr(fd as i32, 0, &t) } != 0 {
        return Err(OsError::of(&std::io::Error::last_os_error()));
    }
    Ok(())
}

/// `TermIsKnownToSupportAnsi` and `isatty`, like the VM.
#[cfg(unix)]
fn supports_ansi(fd: i64) -> bool {
    let term = std::env::var("TERM").unwrap_or_default();
    // SAFETY: a query without effects.
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
    // SAFETY: `w` is the `struct winsize`.
    let r = unsafe { ioctl(fd as i32, TIOCGWINSZ, &mut w as *mut Winsize) };
    if r == 0 && (w.columns != 0 || w.line_list != 0) {
        return Ok((i64::from(w.columns), i64::from(w.line_list)));
    }
    Err(OsError::of(&std::io::Error::last_os_error()))
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
            Err(e) => return Err(OsError::of(&e)),
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
    // SAFETY: queries the mode of a process handle.
    if unsafe { GetConsoleMode(default_handle_of(fd), &mut m) } == 0 {
        return Err(OsError::of(&std::io::Error::last_os_error()));
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
    // SAFETY: changes the mode of a process handle.
    if unsafe { SetConsoleMode(default_handle_of(0), new) } == 0 {
        return Err(OsError::of(&std::io::Error::last_os_error()));
    }
    Ok(())
}

/// `ENABLE_VIRTUAL_TERMINAL_INPUT` (input) / `_PROCESSING` (output).
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
    // SAFETY: the structure is plain data; the function fills it.
    let mut i: Info = unsafe { std::mem::zeroed() };
    if unsafe { GetConsoleScreenBufferInfo(default_handle_of(if fd == 1 { 1 } else { 2 }), &mut i) } == 0 {
        return Err(OsError::of(&std::io::Error::last_os_error()));
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
        Err(e) => Err(OsError::of(&e)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stdin_ReadByte(fd: i64) -> i64 {
    dart_or_error(read_stdin_byte(fd), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stdin_GetEchoMode(fd: i64) -> i64 {
    dart_or_error(terminal_mode(fd, TerminalMode::Echo), dart_bool)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stdin_SetEchoMode(fd: i64, enabled_flag: u8) -> i64 {
    dart_true_or_error(change_terminal_mode(fd, TerminalMode::Echo, enabled_flag != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stdin_GetEchoNewlineMode(fd: i64) -> i64 {
    dart_or_error(terminal_mode(fd, TerminalMode::EchoNewline), dart_bool)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stdin_SetEchoNewlineMode(fd: i64, enabled_flag: u8) -> i64 {
    dart_true_or_error(change_terminal_mode(fd, TerminalMode::EchoNewline, enabled_flag != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stdin_GetLineMode(fd: i64) -> i64 {
    dart_or_error(terminal_mode(fd, TerminalMode::Line), dart_bool)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stdin_SetLineMode(fd: i64, enabled_flag: u8) -> i64 {
    dart_true_or_error(change_terminal_mode(fd, TerminalMode::Line, enabled_flag != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stdin_AnsiSupported(fd: i64) -> i64 {
    dart_bool(supports_ansi(fd))
}

/// `Stdout_GetTerminalSize`: `[columns, lines]` or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stdout_GetTerminalSize(fd: i64) -> i64 {
    dart_or_error(terminal_size(fd), |(c, l)| dart_fixed_list(&[dart_int(c), dart_int(l)]))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Stdout_AnsiSupported(fd: i64) -> i64 {
    dart_bool(supports_ansi(fd))
}

// ---------------------------------------------------------------------------
// The current process (`exit`, `exitCode`, `sleep`, `pid`).

/// The global exit code (`Process::GlobalExitCode`), which the normal end of the
/// program returns.
static EXIT_CODE: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// The code the process ends with when the program finishes without an error.
fn global_exit_code() -> i32 {
    EXIT_CODE.load(std::sync::atomic::Ordering::Relaxed) as i32
}

/// `Process_Exit`: ends the process right away (the VM uses `_exit`, without the
/// global destructors; `std`'s standard output is flushed first).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Process_Exit(status: i64) {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    std::process::exit(status as i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Process_SetExitCode(status: i64) {
    EXIT_CODE.store(status, std::sync::atomic::Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Process_GetExitCode() -> i64 {
    EXIT_CODE.load(std::sync::atomic::Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Process_Sleep(ms: i64) {
    std::thread::sleep(std::time::Duration::from_millis(ms.max(0) as u64));
}

/// `Process_Pid(process)`: that of the current process (with `null`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Process_Pid(process: i64) -> i64 {
    if process == 0 {
        return i64::from(std::process::id());
    }
    native_field(process)
}

// ---------------------------------------------------------------------------
// The system encoding (`SystemEncodingToString`/`StringToSystemEncoding`, the
// VM's `StringUtils::ConsoleStringToUtf8`): UTF-8 on Unix; the console code page
// (`GetConsoleCP`) on Windows.

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
    // SAFETY: sizes queried before; the buffers have the requested size.
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
    // SAFETY: sizes queried before; the buffers have the requested size.
    unsafe {
        let cp = GetConsoleCP();
        let n = WideCharToMultiByte(cp, 0, w.as_ptr(), w.len() as i32, std::ptr::null_mut(), 0, std::ptr::null(), std::ptr::null_mut());
        let mut b = vec![0u8; n.max(0) as usize];
        WideCharToMultiByte(cp, 0, w.as_ptr(), w.len() as i32, b.as_mut_ptr(), n, std::ptr::null(), std::ptr::null_mut());
        b
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SystemEncodingToString(bytes: i64) -> i64 {
    let b = typed_list_bytes(bytes).unwrap_or_else(|| {
        HEAP.with(|h| {
            let h = h.borrow();
            bytes_from_value_list(&h, bytes)
        })
    });
    allocate_str(&system_to_utf8(&b))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_StringToSystemEncoding(text: i64) -> i64 {
    dart_bytes(utf8_to_system(&utf8_of_string(text)))
}
