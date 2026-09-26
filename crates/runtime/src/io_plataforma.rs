// Runtime nativo: o `dart:io` da VM — plataforma, terminal e processo
// (`runtime/bin/platform*.cc`, `stdio*.cc`, `crypto*.cc` e os natives
// simples de `process.cc`): `Platform`, `Stdin`/`Stdout`, `exit`,
// `exitCode`, `sleep`, `pid` e os bytes aleatórios do sistema.

// ---------------------------------------------------------------------------
// Bytes aleatórios do sistema (`Crypto::GetRandomBytes`).

/// Preenche `destino` com bytes do gerador seguro do sistema:
/// `getentropy` no Unix (em blocos de 256, o máximo por chamada) e
/// `BCryptGenRandom` no Windows.
#[cfg(unix)]
fn bytes_aleatorios_do_sistema(destino: &mut [u8]) -> ResultadoIo<()> {
    unsafe extern "C" {
        fn getentropy(buf: *mut std::ffi::c_void, n: usize) -> i32;
    }
    for bloco in destino.chunks_mut(256) {
        // SAFETY: `bloco` é um trecho gravável de até 256 bytes.
        if unsafe { getentropy(bloco.as_mut_ptr().cast(), bloco.len()) } != 0 {
            return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
        }
    }
    Ok(())
}
#[cfg(windows)]
fn bytes_aleatorios_do_sistema(destino: &mut [u8]) -> ResultadoIo<()> {
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptGenRandom(alg: *mut std::ffi::c_void, buf: *mut u8, n: u32, flags: u32) -> i32;
    }
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 2;
    for bloco in destino.chunks_mut(u32::MAX as usize) {
        // SAFETY: `bloco` é gravável com o tamanho passado.
        let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), bloco.as_mut_ptr(), bloco.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
        if status != 0 {
            return Err(ErroDoSo::do_codigo(status));
        }
    }
    Ok(())
}

/// `Crypto_GetRandomBytes(count)`: `count` (0 a 4096) bytes aleatórios.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Crypto_GetRandomBytes(n: i64) -> i64 {
    if !(0..=4096).contains(&n) {
        let msg = alocar_str("Invalid argument: count must be a positive int less than or equal to 4096.");
        com_raizes(&[msg], || dartforge_exception_throw(msg, 3));
        return 0;
    }
    let mut b = vec![0u8; n as usize];
    match bytes_aleatorios_do_sistema(&mut b) {
        Ok(()) => dart_bytes(b),
        Err(e) => {
            let erro = e.para_dart();
            com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
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
    // SAFETY: registrada pela biblioteca `dart:io` da sobreposição com a
    // assinatura (`String`) → `Object`.
    let iniciar: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(iniciar) };
    let script = std::env::args_os().next().map(|a| bytes_de_caminho(std::path::Path::new(&a))).unwrap_or_default();
    let script = dart_texto_de_bytes(&script);
    let gancho = com_raizes(&[script], || iniciar(script));
    if dartforge_exception_pending() != 0 {
        return;
    }
    if let Some(definir) = ajudante("_dartforgeDefinirUriBase") {
        // SAFETY: registrada pela biblioteca `dart:core` da sobreposição com
        // a assinatura (`Uri Function()`) → `void`.
        let definir: extern "C" fn(i64) = unsafe { std::mem::transmute(definir) };
        com_raizes(&[gancho], || definir(gancho));
    }
}

// ---------------------------------------------------------------------------
// `Platform`.

/// A versão do SDK Dart que o programa compilou (`Platform.version`),
/// gravada pela entrada do programa.
static VERSAO_DO_SDK: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Registra a versão do SDK (o arquivo `version` dele).
///
/// # Safety
/// `texto` aponta para `n` bytes UTF-8 legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_versao_do_sdk(texto: *const u8, n: i64) {
    // SAFETY: constante do módulo com `n` bytes.
    let b = unsafe { std::slice::from_raw_parts(texto, n as usize) };
    let _ = VERSAO_DO_SDK.set(String::from_utf8_lossy(b).into_owned());
}

/// O nome do sistema como o Dart o informa (`Platform.operatingSystem`).
const NOME_DO_SISTEMA: &str = if cfg!(target_os = "linux") {
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
const NOME_DA_ARQUITETURA: &str = if cfg!(target_arch = "x86_64") {
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
fn numero_de_processadores() -> i64 {
    unsafe extern "C" {
        fn sysconf(nome: i32) -> std::ffi::c_long;
    }
    #[cfg(target_os = "linux")]
    const SC_NPROCESSORS_ONLN: i32 = 84;
    #[cfg(not(target_os = "linux"))]
    const SC_NPROCESSORS_ONLN: i32 = 58;
    // SAFETY: consulta sem efeitos.
    unsafe { sysconf(SC_NPROCESSORS_ONLN) as i64 }
}
#[cfg(windows)]
fn numero_de_processadores() -> i64 {
    #[repr(C)]
    struct SystemInfo {
        arquitetura: u16,
        reservado: u16,
        pagina: u32,
        minimo: *mut std::ffi::c_void,
        maximo: *mut std::ffi::c_void,
        mascara: usize,
        processadores: u32,
        tipo: u32,
        granularidade: u32,
        nivel: u16,
        revisao: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemInfo(i: *mut SystemInfo);
    }
    // SAFETY: a estrutura é só dados; a função a preenche.
    let mut i: SystemInfo = unsafe { std::mem::zeroed() };
    unsafe { GetSystemInfo(&mut i) };
    i64::from(i.processadores)
}

/// `Platform::OperatingSystemVersion`: `uname` no Linux ("sysname release
/// version"), o texto do `NSProcessInfo` no macOS ("Version 14.5 (Build
/// 23F79)"), e o do registro no Windows ("\"Windows 10 Pro\" 10.0 (Build
/// 19045)").
#[cfg(all(unix, not(target_os = "macos")))]
fn versao_do_sistema() -> ResultadoIo<String> {
    #[repr(C)]
    struct Utsname {
        campos: [[u8; 65]; 6],
    }
    unsafe extern "C" {
        fn uname(u: *mut Utsname) -> i32;
    }
    let mut u = Utsname { campos: [[0; 65]; 6] };
    // SAFETY: `u` tem o tamanho da `struct utsname` do Linux.
    if unsafe { uname(&mut u) } != 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    let campo = |i: usize| {
        let c = &u.campos[i];
        String::from_utf8_lossy(&c[..c.iter().position(|&b| b == 0).unwrap_or(c.len())]).into_owned()
    };
    Ok(format!("{} {} {}", campo(0), campo(2), campo(3)))
}
#[cfg(target_os = "macos")]
fn versao_do_sistema() -> ResultadoIo<String> {
    fn sysctl_texto(nome: &std::ffi::CStr) -> ResultadoIo<String> {
        unsafe extern "C" {
            fn sysctlbyname(nome: *const std::ffi::c_char, v: *mut std::ffi::c_void, n: *mut usize, novo: *mut std::ffi::c_void, nn: usize) -> i32;
        }
        let mut n = 0usize;
        // SAFETY: primeiro o tamanho, depois o valor num buffer desse tamanho.
        if unsafe { sysctlbyname(nome.as_ptr(), std::ptr::null_mut(), &mut n, std::ptr::null_mut(), 0) } != 0 {
            return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
        }
        let mut b = vec![0u8; n];
        if unsafe { sysctlbyname(nome.as_ptr(), b.as_mut_ptr().cast(), &mut n, std::ptr::null_mut(), 0) } != 0 {
            return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
        }
        b.truncate(b.iter().position(|&c| c == 0).unwrap_or(n));
        Ok(String::from_utf8_lossy(&b).into_owned())
    }
    Ok(format!("Version {} (Build {})", sysctl_texto(c"kern.osproductversion")?, sysctl_texto(c"kern.osversion")?))
}
#[cfg(windows)]
fn versao_do_sistema() -> ResultadoIo<String> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(
            chave: isize,
            sub: *const u16,
            valor: *const u16,
            flags: u32,
            tipo: *mut u32,
            dados: *mut std::ffi::c_void,
            n: *mut u32,
        ) -> i32;
    }
    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as i32 as isize;
    const RRF_RT_REG_SZ: u32 = 0x2;
    const RRF_RT_REG_DWORD: u32 = 0x10;
    let largo = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let sub = largo("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
    let texto = |nome: &str| -> ResultadoIo<String> {
        let mut b = [0u16; 256];
        let mut n = std::mem::size_of_val(&b) as u32;
        // SAFETY: o buffer tem `n` bytes.
        let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, sub.as_ptr(), largo(nome).as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), b.as_mut_ptr().cast(), &mut n) };
        if r != 0 {
            return Err(ErroDoSo::do_codigo(r));
        }
        let fim = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        Ok(String::from_utf16_lossy(&b[..fim]))
    };
    let palavra = |nome: &str| -> Option<u32> {
        let mut v = 0u32;
        let mut n = 4u32;
        // SAFETY: `v` tem 4 bytes.
        let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, sub.as_ptr(), largo(nome).as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut v as *mut u32).cast(), &mut n) };
        (r == 0).then_some(v)
    };
    let nome = texto("ProductName")?;
    let numero = match palavra("CurrentMajorVersionNumber") {
        Some(maior) => format!("{maior}.{}", palavra("CurrentMinorVersionNumber").ok_or_else(|| ErroDoSo::do_codigo(2))?),
        None => texto("CurrentVersion")?,
    };
    Ok(format!("\"{nome}\" {numero} (Build {})", texto("CurrentBuild")?))
}

/// `Platform::LocalHostname`.
#[cfg(unix)]
fn nome_do_host() -> ResultadoIo<String> {
    unsafe extern "C" {
        fn gethostname(nome: *mut std::ffi::c_char, n: usize) -> i32;
    }
    let mut b = [0u8; 256];
    // SAFETY: o buffer tem 256 bytes.
    if unsafe { gethostname(b.as_mut_ptr().cast(), b.len()) } != 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    let fim = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    Ok(String::from_utf8_lossy(&b[..fim]).into_owned())
}
#[cfg(windows)]
fn nome_do_host() -> ResultadoIo<String> {
    #[link(name = "ws2_32")]
    unsafe extern "system" {
        fn WSAStartup(versao: u16, dados: *mut u8) -> i32;
        fn GetHostNameW(nome: *mut u16, n: i32) -> i32;
        fn WSAGetLastError() -> i32;
    }
    let mut dados = [0u8; 512];
    // SAFETY: `WSADATA` cabe em 512 bytes; a inicialização é contada.
    if unsafe { WSAStartup(0x0202, dados.as_mut_ptr()) } != 0 {
        return Err(ErroDoSo::do_codigo(unsafe { WSAGetLastError() }));
    }
    let mut b = [0u16; 256];
    // SAFETY: o buffer tem 256 unidades.
    if unsafe { GetHostNameW(b.as_mut_ptr(), b.len() as i32) } != 0 {
        return Err(ErroDoSo::do_codigo(unsafe { WSAGetLastError() }));
    }
    let fim = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    Ok(String::from_utf16_lossy(&b[..fim]))
}

/// `Platform::LocaleName`: `LANG` (ou `en_US`) no Linux; a língua preferida
/// (ou a localidade corrente) no macOS; `GetUserDefaultLocaleName` no
/// Windows.
#[cfg(all(unix, not(target_os = "macos")))]
fn nome_da_localidade() -> ResultadoIo<String> {
    Ok(std::env::var("LANG").unwrap_or_else(|_| "en_US".to_string()))
}
#[cfg(target_os = "macos")]
fn nome_da_localidade() -> ResultadoIo<String> {
    type Ref = *const std::ffi::c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFLocaleCopyPreferredLanguages() -> Ref;
        fn CFArrayGetCount(a: Ref) -> isize;
        fn CFArrayGetValueAtIndex(a: Ref, i: isize) -> Ref;
        fn CFLocaleCopyCurrent() -> Ref;
        fn CFLocaleGetIdentifier(l: Ref) -> Ref;
        fn CFStringGetLength(s: Ref) -> isize;
        fn CFStringGetMaximumSizeForEncoding(n: isize, codificacao: u32) -> isize;
        fn CFStringGetCString(s: Ref, b: *mut std::ffi::c_char, n: isize, codificacao: u32) -> u8;
        fn CFRelease(r: Ref);
    }
    const UTF8: u32 = 0x0800_0100;
    fn texto(s: Ref) -> Option<String> {
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
        let linguas = CFLocaleCopyPreferredLanguages();
        let preferida = if CFArrayGetCount(linguas) >= 1 { texto(CFArrayGetValueAtIndex(linguas, 0)) } else { None };
        CFRelease(linguas);
        if let Some(l) = preferida {
            return Ok(l);
        }
        let local = CFLocaleCopyCurrent();
        let r = texto(CFLocaleGetIdentifier(local));
        CFRelease(local);
        r.ok_or_else(|| ErroDoSo::do_codigo(codigo_do_so::INVALIDO))
    }
}
#[cfg(windows)]
fn nome_da_localidade() -> ResultadoIo<String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultLocaleName(b: *mut u16, n: i32) -> i32;
    }
    let mut b = [0u16; 85];
    // SAFETY: o buffer tem `LOCALE_NAME_MAX_LENGTH` unidades.
    let n = unsafe { GetUserDefaultLocaleName(b.as_mut_ptr(), b.len() as i32) };
    if n == 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    Ok(String::from_utf16_lossy(&b[..(n as usize).saturating_sub(1)]))
}

/// As variáveis de ambiente como `NOME=valor` (as que não são UTF-8 válido
/// ficam de fora, como na VM; no Windows, também as que começam com `=`).
fn variaveis_de_ambiente() -> Vec<String> {
    let mut saida = Vec::new();
    for (k, v) in std::env::vars_os() {
        #[cfg(windows)]
        if k.to_string_lossy().starts_with('=') {
            continue;
        }
        if let (Some(k), Some(v)) = (k.to_str(), v.to_str()) {
            saida.push(format!("{k}={v}"));
        }
    }
    saida
}

/// Uma `_List` de tamanho fixo com as strings; `tipada`: `List<String>`
/// (o `Dart_NewListOfTypeFilled` da VM), senão `List<dynamic>`.
fn dart_lista_de_textos(textos: &[String], tipada: bool) -> i64 {
    let lista = dart_lista_fixa(&vec![0; textos.len()]);
    com_raizes(&[lista], || {
        for (i, t) in textos.iter().enumerate() {
            let s = alocar_str(t);
            HEAP.with(|h| h.borrow_mut().list_set(lista, i, TaggedValue::reference(s)));
        }
        if tipada {
            if let Some(tipo) = tipo_lista_de_textos(cid_do_runtime(lista)) {
                HEAP.with(|h| h.borrow_mut().set_metadado(lista, tipo + 1));
            }
        }
    });
    lista
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_NumberOfProcessors() -> i64 {
    numero_de_processadores()
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_PathSeparator() -> i64 {
    alocar_str(SEPARADOR_DE_CAMINHO)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_OperatingSystem() -> i64 {
    alocar_str(NOME_DO_SISTEMA)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_OperatingSystemVersion() -> i64 {
    dart_ou_erro(versao_do_sistema(), |v| alocar_str(&v))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_LocalHostname() -> i64 {
    dart_ou_erro(nome_do_host(), |v| alocar_str(&v))
}

/// `Platform_ExecutableName`: o `argv[0]` do processo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_ExecutableName() -> i64 {
    match std::env::args_os().next() {
        Some(a) => dart_texto_de_bytes(&bytes_de_caminho(std::path::Path::new(&a))),
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
            dart_texto_de_bytes(&bytes_de_caminho(&p))
        }
        Err(_) => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_Environment() -> i64 {
    dart_lista_de_textos(&variaveis_de_ambiente(), false)
}

/// `Platform_ExecutableArguments`: as opções da VM antes do script — um
/// executável AOT não tem nenhuma.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_ExecutableArguments() -> i64 {
    dart_lista_de_textos(&[], true)
}

/// `Platform_GetVersion`: a versão do SDK, o canal e o alvo, na forma do
/// `Dart_VersionString` da VM (sem a data de compilação da VM, que não
/// existe aqui).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_GetVersion() -> i64 {
    let versao = VERSAO_DO_SDK.get().map_or("unknown", String::as_str);
    let canal = if versao.contains("-edge") {
        "edge"
    } else if versao.contains(".dev") || versao.contains("-dev") {
        "dev"
    } else if versao.contains(".beta") || versao.contains("-beta") {
        "beta"
    } else {
        "stable"
    };
    alocar_str(&format!("{versao} ({canal}) on \"{NOME_DO_SISTEMA}_{NOME_DA_ARQUITETURA}\""))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Platform_LocaleName() -> i64 {
    dart_ou_erro(nome_da_localidade(), |v| alocar_str(&v))
}

// ---------------------------------------------------------------------------
// O terminal (`Stdin`/`Stdout`).

/// Os modos de `Stdin`: eco, eco da nova linha e modo de linha.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ModoDoTerminal {
    Eco,
    EcoDeNovaLinha,
    Linha,
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
    fn gravar_lflag(&mut self, v: u64) {
        self.0[12..16].copy_from_slice(&(v as u32).to_ne_bytes());
    }
    #[cfg(not(target_os = "linux"))]
    fn lflag(&self) -> u64 {
        u64::from_ne_bytes(self.0[24..32].try_into().expect("8 bytes"))
    }
    #[cfg(not(target_os = "linux"))]
    fn gravar_lflag(&mut self, v: u64) {
        self.0[24..32].copy_from_slice(&v.to_ne_bytes());
    }
}

#[cfg(unix)]
fn bit_do_modo(m: ModoDoTerminal) -> u64 {
    #[cfg(target_os = "linux")]
    let (eco, eco_nl, linha) = (0o10, 0o100, 0o2);
    #[cfg(not(target_os = "linux"))]
    let (eco, eco_nl, linha) = (0x8, 0x10, 0x100);
    match m {
        ModoDoTerminal::Eco => eco,
        ModoDoTerminal::EcoDeNovaLinha => eco_nl,
        ModoDoTerminal::Linha => linha,
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn tcgetattr(fd: i32, t: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, acao: i32, t: *const Termios) -> i32;
    fn isatty(fd: i32) -> i32;
    fn ioctl(fd: i32, pedido: std::ffi::c_ulong, ...) -> i32;
}

#[cfg(unix)]
fn atributos_do_terminal(fd: i64) -> ResultadoIo<Termios> {
    let mut t = Termios([0; 128]);
    // SAFETY: `t` é maior que a `struct termios` de qualquer plataforma.
    if unsafe { tcgetattr(fd as i32, &mut t) } != 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    Ok(t)
}

#[cfg(unix)]
fn modo_do_terminal(fd: i64, m: ModoDoTerminal) -> ResultadoIo<bool> {
    Ok(atributos_do_terminal(fd)?.lflag() & bit_do_modo(m) != 0)
}

#[cfg(unix)]
fn mudar_modo_do_terminal(fd: i64, m: ModoDoTerminal, ligado: bool) -> ResultadoIo<()> {
    let mut t = atributos_do_terminal(fd)?;
    let bit = bit_do_modo(m);
    let f = t.lflag();
    t.gravar_lflag(if ligado { f | bit } else { f & !bit });
    // SAFETY: `t` veio de `tcgetattr` (TCSANOW = 0).
    if unsafe { tcsetattr(fd as i32, 0, &t) } != 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    Ok(())
}

/// `TermIsKnownToSupportAnsi` e `isatty`, como a VM.
#[cfg(unix)]
fn suporta_ansi(fd: i64) -> bool {
    let termo = std::env::var("TERM").unwrap_or_default();
    // SAFETY: consulta sem efeitos.
    (unsafe { isatty(fd as i32) } != 0) && ["xterm", "screen", "rxvt"].iter().any(|t| termo.contains(t))
}

#[cfg(unix)]
fn tamanho_do_terminal(fd: i64) -> ResultadoIo<(i64, i64)> {
    #[repr(C)]
    struct Winsize {
        linhas: u16,
        colunas: u16,
        x: u16,
        y: u16,
    }
    #[cfg(target_os = "linux")]
    const TIOCGWINSZ: std::ffi::c_ulong = 0x5413;
    #[cfg(not(target_os = "linux"))]
    const TIOCGWINSZ: std::ffi::c_ulong = 0x4008_7468;
    let mut w = Winsize { linhas: 0, colunas: 0, x: 0, y: 0 };
    // SAFETY: `w` é a `struct winsize`.
    let r = unsafe { ioctl(fd as i32, TIOCGWINSZ, &mut w as *mut Winsize) };
    if r == 0 && (w.colunas != 0 || w.linhas != 0) {
        return Ok((i64::from(w.colunas), i64::from(w.linhas)));
    }
    Err(ErroDoSo::de(&std::io::Error::last_os_error()))
}

#[cfg(unix)]
fn ler_byte_da_entrada(fd: i64) -> ResultadoIo<i64> {
    let f = std::mem::ManuallyDrop::new(arquivo_do_descritor(fd));
    let mut b = [0u8; 1];
    loop {
        use std::io::Read;
        match (&*f).read(&mut b) {
            Ok(0) => return Ok(-1),
            Ok(_) => return Ok(i64::from(b[0])),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(ErroDoSo::de(&e)),
        }
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleMode(h: *mut std::ffi::c_void, modo: *mut u32) -> i32;
    fn SetConsoleMode(h: *mut std::ffi::c_void, modo: u32) -> i32;
}

#[cfg(windows)]
fn handle_padrao(fd: i64) -> *mut std::ffi::c_void {
    descritor_padrao(fd) as usize as *mut std::ffi::c_void
}

#[cfg(windows)]
fn modo_do_console(fd: i64) -> ResultadoIo<u32> {
    let mut m = 0u32;
    // SAFETY: consulta o modo de um handle do processo.
    if unsafe { GetConsoleMode(handle_padrao(fd), &mut m) } == 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    Ok(m)
}

#[cfg(windows)]
fn modo_do_terminal(_fd: i64, m: ModoDoTerminal) -> ResultadoIo<bool> {
    match m {
        ModoDoTerminal::Eco => Ok(modo_do_console(0)? & 0x4 != 0), // ENABLE_ECHO_INPUT
        ModoDoTerminal::Linha => Ok(modo_do_console(0)? & 0x2 != 0), // ENABLE_LINE_INPUT
        ModoDoTerminal::EcoDeNovaLinha => Ok(false),
    }
}

#[cfg(windows)]
fn mudar_modo_do_terminal(_fd: i64, m: ModoDoTerminal, ligado: bool) -> ResultadoIo<()> {
    let bit = match m {
        ModoDoTerminal::Eco => 0x4,
        ModoDoTerminal::Linha => 0x2,
        ModoDoTerminal::EcoDeNovaLinha if ligado => return Err(ErroDoSo::do_codigo(775 /* ERROR_NOT_CAPABLE */)),
        ModoDoTerminal::EcoDeNovaLinha => return Ok(()),
    };
    let atual = modo_do_console(0)?;
    let novo = if ligado { atual | bit } else { atual & !bit };
    // SAFETY: muda o modo de um handle do processo.
    if unsafe { SetConsoleMode(handle_padrao(0), novo) } == 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    Ok(())
}

/// `ENABLE_VIRTUAL_TERMINAL_INPUT` (entrada) / `_PROCESSING` (saída).
#[cfg(windows)]
fn suporta_ansi(fd: i64) -> bool {
    let bit = if fd == 0 { 0x200 } else { 0x4 };
    modo_do_console(fd).is_ok_and(|m| m & bit != 0)
}

#[cfg(windows)]
fn tamanho_do_terminal(fd: i64) -> ResultadoIo<(i64, i64)> {
    #[repr(C)]
    struct Info {
        tamanho: [i16; 2],
        cursor: [i16; 2],
        atributos: u16,
        janela: [i16; 4],
        maximo: [i16; 2],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleScreenBufferInfo(h: *mut std::ffi::c_void, i: *mut Info) -> i32;
    }
    // SAFETY: a estrutura é só dados; a função a preenche.
    let mut i: Info = unsafe { std::mem::zeroed() };
    if unsafe { GetConsoleScreenBufferInfo(handle_padrao(if fd == 1 { 1 } else { 2 }), &mut i) } == 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    let [esquerda, topo, direita, base] = i.janela;
    Ok((i64::from(direita - esquerda + 1), i64::from(base - topo + 1)))
}

#[cfg(windows)]
fn ler_byte_da_entrada(_fd: i64) -> ResultadoIo<i64> {
    let f = std::mem::ManuallyDrop::new(arquivo_do_descritor(descritor_padrao(0)));
    let mut b = [0u8; 1];
    use std::io::Read;
    match (&*f).read(&mut b) {
        Ok(0) => Ok(-1),
        Ok(_) => Ok(i64::from(b[0])),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(-1),
        Err(e) => Err(ErroDoSo::de(&e)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_ReadByte(fd: i64) -> i64 {
    dart_ou_erro(ler_byte_da_entrada(fd), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_GetEchoMode(fd: i64) -> i64 {
    dart_ou_erro(modo_do_terminal(fd, ModoDoTerminal::Eco), dart_bool)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_SetEchoMode(fd: i64, ligado: u8) -> i64 {
    dart_verdadeiro_ou_erro(mudar_modo_do_terminal(fd, ModoDoTerminal::Eco, ligado != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_GetEchoNewlineMode(fd: i64) -> i64 {
    dart_ou_erro(modo_do_terminal(fd, ModoDoTerminal::EcoDeNovaLinha), dart_bool)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_SetEchoNewlineMode(fd: i64, ligado: u8) -> i64 {
    dart_verdadeiro_ou_erro(mudar_modo_do_terminal(fd, ModoDoTerminal::EcoDeNovaLinha, ligado != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_GetLineMode(fd: i64) -> i64 {
    dart_ou_erro(modo_do_terminal(fd, ModoDoTerminal::Linha), dart_bool)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_SetLineMode(fd: i64, ligado: u8) -> i64 {
    dart_verdadeiro_ou_erro(mudar_modo_do_terminal(fd, ModoDoTerminal::Linha, ligado != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdin_AnsiSupported(fd: i64) -> i64 {
    dart_bool(suporta_ansi(fd))
}

/// `Stdout_GetTerminalSize`: `[colunas, linhas]` ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdout_GetTerminalSize(fd: i64) -> i64 {
    dart_ou_erro(tamanho_do_terminal(fd), |(c, l)| dart_lista_fixa(&[dart_int(c), dart_int(l)]))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Stdout_AnsiSupported(fd: i64) -> i64 {
    dart_bool(suporta_ansi(fd))
}

// ---------------------------------------------------------------------------
// O processo corrente (`exit`, `exitCode`, `sleep`, `pid`).

/// O código de saída global (`Process::GlobalExitCode`), que o fim normal
/// do programa devolve.
static CODIGO_DE_SAIDA: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// O código com que o processo termina quando o programa acaba sem erro.
fn codigo_de_saida_global() -> i32 {
    CODIGO_DE_SAIDA.load(std::sync::atomic::Ordering::Relaxed) as i32
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
    CODIGO_DE_SAIDA.store(status, std::sync::atomic::Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_GetExitCode() -> i64 {
    CODIGO_DE_SAIDA.load(std::sync::atomic::Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_Sleep(ms: i64) {
    std::thread::sleep(std::time::Duration::from_millis(ms.max(0) as u64));
}

/// `Process_Pid(process)`: o do processo corrente (com `null`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_Pid(processo: i64) -> i64 {
    if processo == 0 {
        return i64::from(std::process::id());
    }
    campo_nativo(processo)
}

// ---------------------------------------------------------------------------
// A codificação do sistema (`SystemEncodingToString`/`StringToSystemEncoding`,
// o `StringUtils::ConsoleStringToUtf8` da VM): UTF-8 no Unix; a página de
// código do console (`GetConsoleCP`) no Windows.

#[cfg(unix)]
fn do_sistema_para_utf8(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}
#[cfg(unix)]
fn de_utf8_para_o_sistema(s: &[u8]) -> Vec<u8> {
    s.to_vec()
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleCP() -> u32;
    fn MultiByteToWideChar(cp: u32, flags: u32, s: *const u8, n: i32, w: *mut u16, nw: i32) -> i32;
    fn WideCharToMultiByte(cp: u32, flags: u32, w: *const u16, nw: i32, s: *mut u8, n: i32, padrao: *const u8, usou: *mut i32) -> i32;
}
#[cfg(windows)]
fn do_sistema_para_utf8(b: &[u8]) -> String {
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
fn de_utf8_para_o_sistema(s: &[u8]) -> Vec<u8> {
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
    let b = bytes_da_lista_tipada(bytes).unwrap_or_else(|| {
        HEAP.with(|h| {
            let h = h.borrow();
            (0..h.list_len(bytes)).map(|i| h.list_get(bytes, i).bits as u8).collect()
        })
    });
    alocar_str(&do_sistema_para_utf8(&b))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StringToSystemEncoding(texto: i64) -> i64 {
    dart_bytes(de_utf8_para_o_sistema(&utf8_de_texto(texto)))
}
