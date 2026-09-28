// Runtime nativo: a observação de arquivos do `dart:io`
// (`FileSystemEntity.watch`, `runtime/bin/file_system_watcher_*.cc` da VM).
//
// O contrato é o do `_FileSystemWatcher` de `file_patch.dart`, que não é
// substituído: `_initWatcher` devolve o descritor do observador, o
// `_listenOnSocket` o entrega ao manipulador de eventos como soquete interno
// (`_NativeSocket.watch`) e, a cada leitura pronta, `_readEvents` devolve os
// eventos como `[máscara, cookie, nome ou null, é destino de mudança, id do
// caminho]`, com a máscara nos bits de `FileSystemEvent` (`create` 1,
// `modify` 2, `delete` 4, `move` 8, `_modifyAttributes` 16, `_deleteSelf` 32,
// `_isDir` 64).
//
// Linux: inotify, como a VM. O descritor pertence ao soquete interno que o
// `_listenOnSocket` cria logo depois do `_initWatcher`: é o fechamento dele
// (pelo manipulador de eventos) que libera o descritor, e `_closeWatcher`
// não o fecha de novo — fechar duas vezes poderia fechar um descritor já
// reaproveitado por outra abertura.
//
// Windows: `ReadDirectoryChangesW` na porta de conclusão, como a VM
// (`file_system_watcher_win.cc`). Não há descritor do observador (`Init` é
// 0): cada caminho é um manipulador de diretório (`DirectoryWatchHandle`,
// em `io_windows_eventos.rs`), e o ponteiro dele é o id do caminho e o do
// soquete interno. `_unwatchPath` para a leitura e fecha o handle; a
// referência fica com o soquete, que o Dart fecha logo depois. Os eventos
// não trazem "é diretório" (o `_Win32FileSystemWatcher` consulta o disco)
// nem pares de mudança de nome (o cookie é sempre 1, e o Dart os junta).
//
// macOS: FSEvents, como a VM (`file_system_watcher_macos.cc`). O observador
// (`Init`) é uma thread com um `CFRunLoop`; cada caminho é um
// `FSEventStream` (eventos por arquivo, latência de 0,1 s) agendado nesse
// laço e um pipe: a *callback* grava cada evento no pipe como o `FSEvent` da
// VM (existe, bandeiras, caminho relativo com `PATH_MAX` bytes), e o lado de
// leitura é o soquete interno do caminho. O FSEvents vem do `CoreServices`,
// carregado na primeira observação (`dlopen`): nenhum programa que não
// observa arquivos carrega o framework, e a ligação não o pede.

/// Os bits de `FileSystemEvent` (e os internos do `_FileSystemWatcher`).
#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos", windows))]
#[allow(dead_code)]
mod evento_fs {
    pub const CRIAR: i64 = 1 << 0;
    pub const MODIFICAR_CONTEUDO: i64 = 1 << 1;
    pub const APAGAR: i64 = 1 << 2;
    pub const MOVER: i64 = 1 << 3;
    pub const MODIFICAR_ATRIBUTOS: i64 = 1 << 4;
    pub const APAGAR_O_PROPRIO: i64 = 1 << 5;
    pub const E_DIRETORIO: i64 = 1 << 6;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod inotify {
    pub const IN_MODIFY: u32 = 0x0000_0002;
    pub const IN_ATTRIB: u32 = 0x0000_0004;
    pub const IN_CLOSE_WRITE: u32 = 0x0000_0008;
    pub const IN_MOVED_FROM: u32 = 0x0000_0040;
    pub const IN_MOVED_TO: u32 = 0x0000_0080;
    pub const IN_MOVE: u32 = IN_MOVED_FROM | IN_MOVED_TO;
    pub const IN_CREATE: u32 = 0x0000_0100;
    pub const IN_DELETE: u32 = 0x0000_0200;
    pub const IN_DELETE_SELF: u32 = 0x0000_0400;
    pub const IN_MOVE_SELF: u32 = 0x0000_0800;
    pub const IN_IGNORED: u32 = 0x0000_8000;
    pub const IN_ISDIR: u32 = 0x4000_0000;
    pub const IN_NONBLOCK: i32 = 0o4000;
    pub const IN_CLOEXEC: i32 = 0o2_000_000;
    /// `sizeof(struct inotify_event)`: `wd`, `mask`, `cookie`, `len`.
    pub const TAMANHO_DO_EVENTO: usize = 16;

    unsafe extern "C" {
        pub fn inotify_init1(flags: i32) -> i32;
        pub fn inotify_add_watch(fd: i32, caminho: *const std::ffi::c_char, mascara: u32) -> i32;
        pub fn inotify_rm_watch(fd: i32, wd: i32) -> i32;
    }
}

/// Os filtros do `ReadDirectoryChangesW` e as ações de
/// `FILE_NOTIFY_INFORMATION`.
#[cfg(windows)]
mod notificacao {
    pub const FILE_NOTIFY_CHANGE_FILE_NAME: u32 = 0x0000_0001;
    pub const FILE_NOTIFY_CHANGE_DIR_NAME: u32 = 0x0000_0002;
    pub const FILE_NOTIFY_CHANGE_LAST_WRITE: u32 = 0x0000_0010;
    pub const FILE_ACTION_ADDED: u32 = 1;
    pub const FILE_ACTION_REMOVED: u32 = 2;
    pub const FILE_ACTION_MODIFIED: u32 = 3;
    pub const FILE_ACTION_RENAMED_OLD_NAME: u32 = 4;
    pub const FILE_ACTION_RENAMED_NEW_NAME: u32 = 5;
    /// `NextEntryOffset`, `Action` e `FileNameLength` antes do nome.
    pub const CABECALHO: usize = 12;
}

/// `FileSystemWatcher::IsSupported`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_IsSupported() -> u8 {
    u8::from(cfg!(any(target_os = "linux", target_os = "android", target_os = "macos", windows)))
}

/// `FileSystemWatcher::Init`: o descritor do inotify, não bloqueante; lança
/// o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_InitWatcher() -> i64 {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        // SAFETY: chamada sem ponteiros.
        let fd = unsafe { inotify::inotify_init1(inotify::IN_CLOEXEC | inotify::IN_NONBLOCK) };
        if fd < 0 {
            lancar_os_error(&ultimo_erro());
            return 0;
        }
        i64::from(fd)
    }
    // Windows: `FileSystemWatcher::Init` devolve 0; o observador é cada
    // caminho.
    #[cfg(windows)]
    {
        0
    }
    // macOS: a thread do `CFRunLoop` (`FSEventsWatcher`).
    #[cfg(target_os = "macos")]
    {
        match fsevents::Observador::iniciar() {
            Ok(o) => Box::into_raw(o) as i64,
            Err(e) => {
                lancar_os_error(&e);
                0
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", windows)))]
    {
        lancar_os_error(&ErroDoSo::argumento_invalido());
        0
    }
}

/// `FileSystemWatcher::Close`: no Linux o descritor é do soquete interno
/// (ver o cabeçalho), e no Windows não há observador; no macOS, para o
/// `CFRunLoop` e espera a thread dele.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_CloseWatcher(id: i64) {
    #[cfg(target_os = "macos")]
    if id != 0 {
        // SAFETY: `id` veio do `Box::into_raw` do `InitWatcher`, e o Dart
        // fecha cada observador uma vez.
        drop(unsafe { Box::from_raw(id as *mut fsevents::Observador) });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = id;
}

/// `FileSystemWatcher::WatchPath`: o `wd` do caminho, com os eventos
/// pedidos (e sempre `IN_DELETE_SELF | IN_MOVE_SELF`); lança o `OSError`.
/// O inotify não observa subdiretórios: `recursive` é ignorado, como na VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_WatchPath(id: i64, _ns: i64, caminho: i64, eventos: i64, recursivo: u8) -> i64 {
    let recursivo = recursivo != 0;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let _ = recursivo;
        use inotify::*;
        let mut mascara = IN_DELETE_SELF | IN_MOVE_SELF;
        if eventos & evento_fs::CRIAR != 0 {
            mascara |= IN_CREATE;
        }
        if eventos & evento_fs::MODIFICAR_CONTEUDO != 0 {
            mascara |= IN_CLOSE_WRITE | IN_ATTRIB | IN_MODIFY;
        }
        if eventos & evento_fs::APAGAR != 0 {
            mascara |= IN_DELETE;
        }
        if eventos & evento_fs::MOVER != 0 {
            mascara |= IN_MOVE;
        }
        let Ok(c) = std::ffi::CString::new(utf8_de_texto(caminho)) else {
            lancar_os_error(&ErroDoSo::argumento_invalido());
            return 0;
        };
        // SAFETY: `c` é um caminho C válido.
        let wd = unsafe { inotify_add_watch(id as i32, c.as_ptr(), mascara) };
        if wd < 0 {
            lancar_os_error(&ultimo_erro());
            return 0;
        }
        i64::from(wd)
    }
    #[cfg(windows)]
    {
        use notificacao::*;
        let _ = id;
        let mut filtro = 0;
        if eventos & (evento_fs::CRIAR | evento_fs::MOVER | evento_fs::APAGAR) != 0 {
            filtro |= FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME;
        }
        if eventos & evento_fs::MODIFICAR_CONTEUDO != 0 {
            filtro |= FILE_NOTIFY_CHANGE_LAST_WRITE;
        }
        match observar_diretorio(&String::from_utf8_lossy(&utf8_de_texto(caminho)), filtro, recursivo) {
            Ok(d) => d,
            Err(e) => {
                lancar_os_error(&e);
                0
            }
        }
    }
    // macOS: todos os eventos (o Dart filtra pela máscara do caminho).
    #[cfg(target_os = "macos")]
    {
        let _ = eventos;
        // SAFETY: `id` é o observador vivo do `InitWatcher`.
        let observador = unsafe { &*(id as *const fsevents::Observador) };
        match observador.observar(&utf8_de_texto(caminho), recursivo) {
            Ok(no) => no,
            Err(e) => {
                lancar_os_error(&e);
                0
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", windows)))]
    {
        let _ = (id, caminho, eventos, recursivo);
        lancar_os_error(&ErroDoSo::argumento_invalido());
        0
    }
}

/// `FileSystemWatcher::UnwatchPath`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_UnwatchPath(id: i64, caminho: i64) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    // SAFETY: chamada sem ponteiros; um `wd` já removido só dá `EINVAL`.
    unsafe {
        inotify::inotify_rm_watch(id as i32, caminho as i32);
    }
    #[cfg(windows)]
    {
        let _ = id;
        parar_observacao(caminho);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = id;
        fsevents::parar(caminho);
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", windows)))]
    let _ = (id, caminho);
}

/// `FileSystemWatcher::GetSocketId`: no Linux, o próprio descritor; no
/// Windows, o manipulador do caminho; no macOS, o lado de leitura do pipe do
/// caminho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_GetSocketId(id: i64, caminho: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        let _ = id;
        fsevents::descritor_de_leitura(caminho).map_or(-1, i64::from)
    }
    #[cfg(not(target_os = "macos"))]
    if cfg!(windows) { caminho } else { id }
}

/// A máscara de `FileSystemEvent` de um evento do inotify
/// (`InotifyEventToMask`).
#[cfg(any(target_os = "linux", target_os = "android"))]
fn mascara_do_inotify(m: u32) -> i64 {
    use inotify::*;
    let mut r = 0;
    if m & IN_CLOSE_WRITE != 0 {
        r |= evento_fs::MODIFICAR_CONTEUDO;
    }
    if m & IN_ATTRIB != 0 {
        r |= evento_fs::MODIFICAR_ATRIBUTOS;
    }
    if m & IN_CREATE != 0 {
        r |= evento_fs::CRIAR;
    }
    if m & IN_MOVE != 0 {
        r |= evento_fs::MOVER;
    }
    if m & IN_DELETE != 0 {
        r |= evento_fs::APAGAR;
    }
    if m & (IN_DELETE_SELF | IN_MOVE_SELF) != 0 {
        r |= evento_fs::APAGAR_O_PROPRIO;
    }
    if m & IN_ISDIR != 0 {
        r |= evento_fs::E_DIRETORIO;
    }
    r
}

/// O tamanho da leitura do inotify: vários eventos de uma vez (cada um cabe
/// em `16 + NAME_MAX + 1` bytes, e o sistema nunca parte um evento).
#[cfg(any(target_os = "linux", target_os = "android"))]
const TAMANHO_DA_LEITURA_DE_EVENTOS: usize = 4096;

/// `FileSystemWatcher::ReadEvents`: os eventos prontos (lista vazia quando
/// não há nenhum, o que encerra o laço de leitura do `_listenOnSocket`);
/// lança o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_ReadEvents(id: i64, caminho: i64) -> i64 {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use inotify::*;
        let _ = caminho;
        let mut buf = vec![0u8; TAMANHO_DA_LEITURA_DE_EVENTOS];
        let lidos = match ler_do_soquete(id, &mut buf) {
            Ok(n) => n,
            Err(e) => {
                lancar_os_error(&e);
                return 0;
            }
        };
        let mut eventos: Vec<i64> = Vec::new();
        let mut p = 0;
        while p + TAMANHO_DO_EVENTO <= lidos {
            let campo = |i: usize| u32::from_ne_bytes(buf[p + i..p + i + 4].try_into().unwrap());
            let (wd, mascara, cookie, tamanho) = (campo(0) as i32, campo(4), campo(8), campo(12) as usize);
            let nome = &buf[p + TAMANHO_DO_EVENTO..(p + TAMANHO_DO_EVENTO + tamanho).min(lidos)];
            p += TAMANHO_DO_EVENTO + tamanho;
            if mascara & IN_IGNORED != 0 {
                continue;
            }
            let nome = &nome[..nome.iter().position(|&c| c == 0).unwrap_or(nome.len())];
            let evento = com_raizes(&eventos, || {
                let texto = if tamanho > 0 { alocar_str(&String::from_utf8_lossy(nome)) } else { 0 };
                com_raizes(&[texto], || {
                    dart_lista_fixa(&[
                        dart_int(mascara_do_inotify(mascara)),
                        dart_int(i64::from(cookie)),
                        texto,
                        dart_bool(mascara & IN_MOVED_TO != 0),
                        dart_int(i64::from(wd)),
                    ])
                })
            });
            eventos.push(evento);
        }
        com_raizes(&eventos, || dart_lista_fixa(&eventos))
    }
    #[cfg(windows)]
    {
        use notificacao::*;
        let _ = id;
        let buf = match eventos_do_diretorio(caminho) {
            Ok(b) => b,
            Err(e) => {
                lancar_os_error(&e);
                return 0;
            }
        };
        let campo = |i: usize| u32::from_ne_bytes(buf[i..i + 4].try_into().unwrap());
        let mut eventos: Vec<i64> = Vec::new();
        let mut p = 0;
        while p + CABECALHO <= buf.len() {
            let (proximo, acao, tamanho) = (campo(p) as usize, campo(p + 4), campo(p + 8) as usize);
            let fim = (p + CABECALHO + tamanho).min(buf.len());
            let nome: Vec<u16> = buf[p + CABECALHO..fim].chunks_exact(2).map(|b| u16::from_ne_bytes([b[0], b[1]])).collect();
            let mascara = match acao {
                FILE_ACTION_ADDED => evento_fs::CRIAR,
                FILE_ACTION_REMOVED => evento_fs::APAGAR,
                FILE_ACTION_MODIFIED => evento_fs::MODIFICAR_CONTEUDO,
                FILE_ACTION_RENAMED_OLD_NAME | FILE_ACTION_RENAMED_NEW_NAME => evento_fs::MOVER,
                _ => 0,
            };
            let evento = com_raizes(&eventos, || {
                let texto = alocar_str(&String::from_utf16_lossy(&nome));
                com_raizes(&[texto], || {
                    // O cookie 1 e "é destino" verdadeiro para todos, como
                    // a VM: o Dart junta os pares de mudança de nome.
                    dart_lista_fixa(&[dart_int(mascara), dart_int(1), texto, dart_bool(true), dart_int(caminho)])
                })
            });
            eventos.push(evento);
            if proximo == 0 {
                break;
            }
            p += proximo;
        }
        com_raizes(&eventos, || dart_lista_fixa(&eventos))
    }
    #[cfg(target_os = "macos")]
    {
        let _ = id;
        let Some(fd) = fsevents::descritor_de_leitura(caminho) else {
            return dart_lista_fixa(&[]);
        };
        // Só os registros inteiros (`FDUtils::AvailableBytes` / tamanho),
        // como a VM: um registro maior que `PIPE_BUF` pode chegar em partes.
        let n = (bytes_disponiveis(i64::from(fd)).max(0) as usize) / fsevents::TAMANHO_DO_REGISTRO;
        let mut eventos: Vec<i64> = Vec::new();
        let mut registro = vec![0u8; fsevents::TAMANHO_DO_REGISTRO];
        for _ in 0..n {
            let mut lido = 0;
            while lido < registro.len() {
                match ler_do_soquete(i64::from(fd), &mut registro[lido..]) {
                    Ok(0) => break,
                    Ok(k) => lido += k,
                    Err(e) => {
                        lancar_os_error(&e);
                        return 0;
                    }
                }
            }
            if lido < registro.len() {
                break;
            }
            let (mascara, nome) = fsevents::evento_do_registro(&registro);
            let evento = com_raizes(&eventos, || {
                let texto = alocar_str(&String::from_utf8_lossy(nome));
                com_raizes(&[texto], || {
                    // Cookie 1 e "é destino" verdadeiro, como a VM.
                    dart_lista_fixa(&[dart_int(mascara), dart_int(1), texto, dart_bool(true), dart_int(caminho)])
                })
            });
            eventos.push(evento);
        }
        com_raizes(&eventos, || dart_lista_fixa(&eventos))
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", windows)))]
    {
        let _ = (id, caminho);
        dart_lista_fixa(&[])
    }
}

/// O FSEvents do macOS (`file_system_watcher_macos.cc` da VM).
#[cfg(target_os = "macos")]
mod fsevents {
    use std::ffi::{c_char, c_void};

    type Ref = *const c_void;

    /// `PATH_MAX` do macOS.
    const PATH_MAX: usize = 1024;
    /// O `FSEvent` da VM: `exists`, `flags` e o caminho relativo.
    pub const TAMANHO_DO_REGISTRO: usize = PATH_MAX + 8;

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_FS_EVENT_STREAM_EVENT_ID_SINCE_NOW: u64 = u64::MAX;
    const K_FS_EVENT_STREAM_CREATE_FLAG_FILE_EVENTS: u32 = 0x10;
    const ITEM_CREATED: u32 = 0x100;
    const ITEM_REMOVED: u32 = 0x200;
    const ITEM_RENAMED: u32 = 0x800;
    const ITEM_MODIFIED: u32 = 0x1000;
    const ITEM_XATTR_MOD: u32 = 0x8000;
    const ITEM_IS_DIR: u32 = 0x2_0000;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(a: Ref, s: *const c_char, codificacao: u32) -> Ref;
        fn CFArrayCreate(a: Ref, valores: *const Ref, n: isize, callbacks: Ref) -> Ref;
        fn CFRelease(r: Ref);
        fn CFRetain(r: Ref) -> Ref;
        fn CFRunLoopGetCurrent() -> Ref;
        fn CFRunLoopRun();
        fn CFRunLoopStop(rl: Ref);
        fn CFRunLoopWakeUp(rl: Ref);
        fn CFRunLoopTimerCreate(
            a: Ref,
            quando: f64,
            intervalo: f64,
            bandeiras: u64,
            ordem: isize,
            cb: extern "C" fn(Ref, *mut c_void),
            contexto: *mut ContextoDoTimer,
        ) -> Ref;
        fn CFRunLoopAddTimer(rl: Ref, t: Ref, modo: Ref);
        fn CFAbsoluteTimeGetCurrent() -> f64;
        static kCFRunLoopDefaultMode: Ref;
        static kCFRunLoopCommonModes: Ref;
    }

    unsafe extern "C" {
        fn dlopen(caminho: *const c_char, modo: i32) -> *mut c_void;
        fn dlsym(h: *mut c_void, nome: *const c_char) -> *mut c_void;
        fn close(fd: i32) -> i32;
    }

    /// `CFRunLoopTimerContext`.
    #[repr(C)]
    struct ContextoDoTimer {
        versao: isize,
        info: *mut c_void,
        retain: Ref,
        release: Ref,
        descricao: Ref,
    }

    /// `FSEventStreamContext`.
    #[repr(C)]
    struct ContextoDoFluxo {
        versao: isize,
        info: *mut c_void,
        retain: Ref,
        release: Option<extern "C" fn(*const c_void)>,
        descricao: Ref,
    }

    type Callback = extern "C" fn(Ref, *mut c_void, usize, *mut c_void, *const u32, *const u64);

    /// As funções do FSEvents (`CoreServices`).
    struct Funcoes {
        criar: unsafe extern "C" fn(Ref, Callback, *mut ContextoDoFluxo, Ref, u64, f64, u32) -> Ref,
        agendar: unsafe extern "C" fn(Ref, Ref, Ref),
        iniciar: unsafe extern "C" fn(Ref) -> u8,
        esvaziar: unsafe extern "C" fn(Ref),
        parar: unsafe extern "C" fn(Ref),
        invalidar: unsafe extern "C" fn(Ref),
        liberar: unsafe extern "C" fn(Ref),
    }

    fn funcoes() -> Result<&'static Funcoes, super::ErroDoSo> {
        static F: std::sync::OnceLock<Option<Funcoes>> = std::sync::OnceLock::new();
        F.get_or_init(|| {
            // SAFETY: caminho C válido; cada símbolo tem a assinatura
            // documentada do FSEvents.
            unsafe {
                let h = dlopen(c"/System/Library/Frameworks/CoreServices.framework/CoreServices".as_ptr(), 1);
                if h.is_null() {
                    return None;
                }
                let s = |n: &std::ffi::CStr| {
                    let p = dlsym(h, n.as_ptr());
                    (!p.is_null()).then_some(p)
                };
                Some(Funcoes {
                    criar: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamCreate")?),
                    agendar: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamScheduleWithRunLoop")?),
                    iniciar: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamStart")?),
                    esvaziar: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamFlushSync")?),
                    parar: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamStop")?),
                    invalidar: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamInvalidate")?),
                    liberar: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamRelease")?),
                })
            }
        })
        .as_ref()
        .ok_or_else(super::ErroDoSo::argumento_invalido)
    }

    /// O `FSEventsWatcher`: a thread do `CFRunLoop` onde os fluxos rodam.
    pub struct Observador {
        laco: usize,
        fio: Option<std::thread::JoinHandle<()>>,
    }

    extern "C" fn nada(_t: Ref, _i: *mut c_void) {}

    extern "C" fn parar_o_laco(_t: Ref, info: *mut c_void) {
        // SAFETY: `info` é o `CFRunLoop` do observador, retido por ele.
        unsafe { CFRunLoopStop(info as Ref) };
    }

    impl Observador {
        /// Sobe a thread e espera o laço dela existir.
        pub fn iniciar() -> Result<Box<Observador>, super::ErroDoSo> {
            let (tx, rx) = std::sync::mpsc::channel::<usize>();
            let fio = std::thread::Builder::new()
                .name("dart:io FileWatcher".into())
                .spawn(move || {
                    // SAFETY: o laço desta thread; o temporizador vazio o
                    // mantém vivo (sem fontes o `CFRunLoopRun` volta logo).
                    unsafe {
                        let laco = CFRunLoopGetCurrent();
                        CFRetain(laco);
                        let t = CFRunLoopTimerCreate(
                            std::ptr::null(),
                            CFAbsoluteTimeGetCurrent() + 1.0,
                            1.0,
                            0,
                            0,
                            nada,
                            std::ptr::null_mut(),
                        );
                        CFRunLoopAddTimer(laco, t, kCFRunLoopCommonModes);
                        CFRelease(t);
                        let _ = tx.send(laco as usize);
                        CFRunLoopRun();
                    }
                })
                .map_err(|e| super::ErroDoSo::de(&e))?;
            let laco = rx.recv().map_err(|_| super::ErroDoSo::argumento_invalido())?;
            Ok(Box::new(Observador { laco, fio: Some(fio) }))
        }

        /// `AddPath`: o pipe, o caminho real e o fluxo; devolve o id do
        /// caminho (o nó).
        pub fn observar(&self, caminho: &[u8], recursivo: bool) -> Result<i64, super::ErroDoSo> {
            use std::os::unix::ffi::OsStrExt;
            let f = funcoes()?;
            let [leitura, escrita] = super::pipe_fechado_no_exec()?;
            super::tornar_nao_bloqueante(leitura);
            // O caminho real (`realpath`): o FSEvents entrega os caminhos já
            // resolvidos (`/private/var/…` para `/var/…`).
            let bruto = std::ffi::OsStr::from_bytes(caminho);
            let real = std::fs::canonicalize(bruto).map(|p| p.as_os_str().as_bytes().to_vec()).unwrap_or_else(|_| caminho.to_vec());
            let c = std::ffi::CString::new(real.clone()).map_err(|_| super::ErroDoSo::argumento_invalido())?;
            // SAFETY: `c` é um texto C válido.
            let texto = unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8) };
            let no = Box::into_raw(Box::new(No { base: real.len(), texto, leitura, escrita, recursivo, fluxo: std::ptr::null() }));
            DESCRITORES.lock().unwrap_or_else(|e| e.into_inner()).insert(no as i64, leitura);
            let mut contexto =
                ContextoDoFluxo { versao: 0, info: no.cast(), retain: std::ptr::null(), release: Some(liberar_no), descricao: std::ptr::null() };
            // SAFETY: as funções do FSEvents com os tipos documentados; o nó
            // vive até a `release` do contexto (o fluxo liberado).
            unsafe {
                let lista = CFArrayCreate(std::ptr::null(), &(*no).texto, 1, std::ptr::null());
                let fluxo = (f.criar)(
                    std::ptr::null(),
                    ao_evento,
                    &mut contexto,
                    lista,
                    K_FS_EVENT_STREAM_EVENT_ID_SINCE_NOW,
                    0.10,
                    K_FS_EVENT_STREAM_CREATE_FLAG_FILE_EVENTS,
                );
                CFRelease(lista);
                (*no).fluxo = fluxo;
                (f.agendar)(fluxo, self.laco as Ref, kCFRunLoopDefaultMode);
                (f.iniciar)(fluxo);
                (f.esvaziar)(fluxo);
            }
            Ok(no as i64)
        }
    }

    impl Drop for Observador {
        /// `Stop`: agenda a parada no próprio laço e espera a thread.
        fn drop(&mut self) {
            let laco = self.laco as Ref;
            let mut contexto =
                ContextoDoTimer { versao: 0, info: laco as *mut c_void, retain: std::ptr::null(), release: std::ptr::null(), descricao: std::ptr::null() };
            // SAFETY: o laço está retido até aqui; o temporizador copia o
            // contexto.
            unsafe {
                let t = CFRunLoopTimerCreate(std::ptr::null(), 0.0, 0.0, 0, 0, parar_o_laco, &mut contexto);
                CFRunLoopAddTimer(laco, t, kCFRunLoopCommonModes);
                CFRelease(t);
                CFRunLoopWakeUp(laco);
            }
            if let Some(f) = self.fio.take() {
                let _ = f.join();
            }
            // SAFETY: a retenção feita pela thread.
            unsafe { CFRelease(laco) };
        }
    }

    /// Um caminho observado (`FSEventsWatcher::Node`).
    struct No {
        /// O tamanho do caminho real (o prefixo tirado de cada evento).
        base: usize,
        texto: Ref,
        leitura: i32,
        escrita: i32,
        recursivo: bool,
        fluxo: Ref,
    }

    /// O lado de leitura do pipe de cada caminho vivo, pelo id: o Dart pode
    /// pedir eventos depois do `UnwatchPath`, e o nó já não existe.
    static DESCRITORES: std::sync::Mutex<std::collections::BTreeMap<i64, i32>> =
        std::sync::Mutex::new(std::collections::BTreeMap::new());

    /// O descritor de leitura do caminho `id`, se ele ainda é observado.
    pub fn descritor_de_leitura(id: i64) -> Option<i32> {
        DESCRITORES.lock().unwrap_or_else(|e| e.into_inner()).get(&id).copied()
    }

    /// `UnwatchPath`: para e libera o fluxo (a `release` do contexto libera o
    /// nó e fecha o lado de escrita do pipe).
    pub fn parar(id: i64) {
        if DESCRITORES.lock().unwrap_or_else(|e| e.into_inner()).remove(&id).is_none() {
            return;
        }
        let Ok(f) = funcoes() else { return };
        // SAFETY: o nó está vivo até a liberação do fluxo, a última chamada.
        unsafe {
            let fluxo = (*(id as *const No)).fluxo;
            (f.parar)(fluxo);
            (f.invalidar)(fluxo);
            (f.liberar)(fluxo);
        }
    }

    extern "C" fn liberar_no(info: *const c_void) {
        // SAFETY: `info` é o `Box` do `observar`, liberado uma vez (quando o
        // fluxo deixa de existir).
        let no = unsafe { Box::from_raw(info as *mut No) };
        // SAFETY: o lado de escrita é só deste nó; o texto foi criado nele.
        unsafe {
            close(no.escrita);
            CFRelease(no.texto);
        }
    }

    /// A *callback* do fluxo, na thread do observador: cada evento vira um
    /// registro no pipe (o caminho relativo ao observado; fora do modo
    /// recursivo, só os do próprio diretório).
    extern "C" fn ao_evento(_fluxo: Ref, info: *mut c_void, n: usize, caminhos: *mut c_void, bandeiras: *const u32, _ids: *const u64) {
        use std::os::unix::ffi::OsStrExt;
        // SAFETY: `info` é o nó vivo (o fluxo o mantém); `caminhos` e
        // `bandeiras` têm `n` itens.
        let no = unsafe { &*(info as *const No) };
        let caminhos = caminhos as *const *const c_char;
        for i in 0..n {
            // SAFETY: ver acima.
            let (c, flags) = unsafe { (std::ffi::CStr::from_ptr(*caminhos.add(i)).to_bytes(), *bandeiras.add(i)) };
            let existe = std::fs::symlink_metadata(std::ffi::OsStr::from_bytes(c)).is_ok();
            let mut resto = &c[no.base.min(c.len())..];
            if !resto.is_empty() {
                resto = &resto[1..];
            }
            if !no.recursivo && resto.contains(&b'/') {
                continue;
            }
            let mut registro = [0u8; TAMANHO_DO_REGISTRO];
            registro[..4].copy_from_slice(&u32::from(existe).to_ne_bytes());
            registro[4..8].copy_from_slice(&flags.to_ne_bytes());
            let k = resto.len().min(PATH_MAX - 1);
            registro[8..8 + k].copy_from_slice(&resto[..k]);
            super::escrever_bloqueante(no.escrita, &registro);
        }
    }

    /// A máscara de `FileSystemEvent` e o nome de um registro
    /// (`FileSystemWatcher::ReadEvents` da VM).
    pub fn evento_do_registro(r: &[u8]) -> (i64, &[u8]) {
        use super::evento_fs::*;
        let existe = u32::from_ne_bytes(r[..4].try_into().unwrap()) != 0;
        let flags = u32::from_ne_bytes(r[4..8].try_into().unwrap());
        let caminho = &r[8..];
        let nome = &caminho[..caminho.iter().position(|&b| b == 0).unwrap_or(caminho.len())];
        let mut mascara = 0;
        if flags & ITEM_RENAMED != 0 {
            mascara |= if nome.is_empty() {
                APAGAR_O_PROPRIO
            } else if existe {
                CRIAR
            } else {
                APAGAR
            };
        }
        if flags & ITEM_MODIFIED != 0 {
            mascara |= MODIFICAR_CONTEUDO;
        }
        if flags & ITEM_XATTR_MOD != 0 {
            mascara |= MODIFICAR_ATRIBUTOS;
        }
        if flags & ITEM_CREATED != 0 {
            mascara |= CRIAR;
        }
        if flags & ITEM_IS_DIR != 0 {
            mascara |= E_DIRETORIO;
        }
        if flags & ITEM_REMOVED != 0 {
            mascara |= if nome.is_empty() { APAGAR_O_PROPRIO } else { APAGAR };
        }
        (mascara, nome)
    }
}
