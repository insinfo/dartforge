// Runtime nativo: a observação de arquivos do `dart:io`
// (`FileSystemEntity.watch`, `runtime/bin/file_system_watcher_*.cc` da VM).
//
// O contrato é o do `_FileSystemWatcher` de `file_patch.dart`, que não é
// substituído: `_initWatcher` devolve o descritor do observador, o
// `_listenOnSocket` o entrega ao manipulador de eventos como soquete interno
// (`_NativeSocket.watch`) e, a cada leitura pronta, `_readEvents` devolve os
// eventos como `[máscara, cookie, nome ou null, é destino de mudança, id do
// caminho]`, com a máscara nodes bits de `FileSystemEvent` (`create` 1,
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
mod fs_event {
    pub const CREATE: i64 = 1 << 0;
    pub const MODIFY_CONTENT: i64 = 1 << 1;
    pub const DELETE: i64 = 1 << 2;
    pub const MOVER: i64 = 1 << 3;
    pub const MODIFY_ATTRIBUTES: i64 = 1 << 4;
    pub const DELETE_SELF: i64 = 1 << 5;
    pub const IS_DIRECTORY: i64 = 1 << 6;
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
    pub const EVENT_SIZE: usize = 16;

    unsafe extern "C" {
        pub fn inotify_init1(flags: i32) -> i32;
        pub fn inotify_add_watch(fd: i32, file_path: *const std::ffi::c_char, mask: u32) -> i32;
        pub fn inotify_rm_watch(fd: i32, wd: i32) -> i32;
    }
}

/// Os filtros do `ReadDirectoryChangesW` e as ações de
/// `FILE_NOTIFY_INFORMATION`.
#[cfg(windows)]
mod notification {
    pub const FILE_NOTIFY_CHANGE_FILE_NAME: u32 = 0x0000_0001;
    pub const FILE_NOTIFY_CHANGE_DIR_NAME: u32 = 0x0000_0002;
    pub const FILE_NOTIFY_CHANGE_LAST_WRITE: u32 = 0x0000_0010;
    pub const FILE_ACTION_ADDED: u32 = 1;
    pub const FILE_ACTION_REMOVED: u32 = 2;
    pub const FILE_ACTION_MODIFIED: u32 = 3;
    pub const FILE_ACTION_RENAMED_OLD_NAME: u32 = 4;
    pub const FILE_ACTION_RENAMED_NEW_NAME: u32 = 5;
    /// `NextEntryOffset`, `Action` e `FileNameLength` antes do nome.
    pub const HEADER: usize = 12;
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
            throw_os_error(&last_error());
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
        match fsevents::Watcher::iniciar() {
            Ok(o) => Box::into_raw(o) as i64,
            Err(e) => {
                throw_os_error(&e);
                0
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", windows)))]
    {
        throw_os_error(&OsError::invalid_argument());
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
        drop(unsafe { Box::from_raw(id as *mut fsevents::Watcher) });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = id;
}

/// `FileSystemWatcher::WatchPath`: o `wd` do caminho, com os eventos
/// pedidos (e sempre `IN_DELETE_SELF | IN_MOVE_SELF`); lança o `OSError`.
/// O inotify não observa subdiretórios: `recursive` é ignorado, como na VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_WatchPath(id: i64, _ns: i64, file_path: i64, events: i64, recursive: u8) -> i64 {
    let recursive = recursive != 0;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let _ = recursive;
        use inotify::*;
        let mut mask = IN_DELETE_SELF | IN_MOVE_SELF;
        if events & fs_event::CREATE != 0 {
            mask |= IN_CREATE;
        }
        if events & fs_event::MODIFY_CONTENT != 0 {
            mask |= IN_CLOSE_WRITE | IN_ATTRIB | IN_MODIFY;
        }
        if events & fs_event::DELETE != 0 {
            mask |= IN_DELETE;
        }
        if events & fs_event::MOVER != 0 {
            mask |= IN_MOVE;
        }
        let Ok(c) = std::ffi::CString::new(utf8_of_string(file_path)) else {
            throw_os_error(&OsError::invalid_argument());
            return 0;
        };
        // SAFETY: `c` é um caminho C válido.
        let wd = unsafe { inotify_add_watch(id as i32, c.as_ptr(), mask) };
        if wd < 0 {
            throw_os_error(&last_error());
            return 0;
        }
        i64::from(wd)
    }
    #[cfg(windows)]
    {
        use notification::*;
        let _ = id;
        let mut filter_obj = 0;
        if events & (fs_event::CREATE | fs_event::MOVER | fs_event::DELETE) != 0 {
            filter_obj |= FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME;
        }
        if events & fs_event::MODIFY_CONTENT != 0 {
            filter_obj |= FILE_NOTIFY_CHANGE_LAST_WRITE;
        }
        match watch_directory(&String::from_utf8_lossy(&utf8_of_string(file_path)), filter_obj, recursive) {
            Ok(d) => d,
            Err(e) => {
                throw_os_error(&e);
                0
            }
        }
    }
    // macOS: todos os eventos (o Dart filtra pela máscara do caminho).
    #[cfg(target_os = "macos")]
    {
        let _ = events;
        // SAFETY: `id` é o observador vivo do `InitWatcher`.
        let observer = unsafe { &*(id as *const fsevents::Watcher) };
        match observer.watch_path(&utf8_of_string(file_path), recursive) {
            Ok(no) => no,
            Err(e) => {
                throw_os_error(&e);
                0
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", windows)))]
    {
        let _ = (id, file_path, events, recursive);
        throw_os_error(&OsError::invalid_argument());
        0
    }
}

/// `FileSystemWatcher::UnwatchPath`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_UnwatchPath(id: i64, file_path: i64) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    // SAFETY: chamada sem ponteiros; um `wd` já removido só dá `EINVAL`.
    unsafe {
        inotify::inotify_rm_watch(id as i32, file_path as i32);
    }
    #[cfg(windows)]
    {
        let _ = id;
        stop_watch(file_path);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = id;
        fsevents::stop(file_path);
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", windows)))]
    let _ = (id, file_path);
}

/// `FileSystemWatcher::GetSocketId`: no Linux, o próprio descritor; no
/// Windows, o manipulador do caminho; no macOS, o lado de leitura do pipe do
/// caminho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_GetSocketId(id: i64, file_path: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        let _ = id;
        fsevents::read_descriptor(file_path).map_or(-1, i64::from)
    }
    #[cfg(not(target_os = "macos"))]
    if cfg!(windows) { file_path } else { id }
}

/// A máscara de `FileSystemEvent` de um evento do inotify
/// (`InotifyEventToMask`).
#[cfg(any(target_os = "linux", target_os = "android"))]
fn inotify_mask(m: u32) -> i64 {
    use inotify::*;
    let mut r = 0;
    if m & IN_CLOSE_WRITE != 0 {
        r |= fs_event::MODIFY_CONTENT;
    }
    if m & IN_ATTRIB != 0 {
        r |= fs_event::MODIFY_ATTRIBUTES;
    }
    if m & IN_CREATE != 0 {
        r |= fs_event::CREATE;
    }
    if m & IN_MOVE != 0 {
        r |= fs_event::MOVER;
    }
    if m & IN_DELETE != 0 {
        r |= fs_event::DELETE;
    }
    if m & (IN_DELETE_SELF | IN_MOVE_SELF) != 0 {
        r |= fs_event::DELETE_SELF;
    }
    if m & IN_ISDIR != 0 {
        r |= fs_event::IS_DIRECTORY;
    }
    r
}

/// O tamanho da leitura do inotify: vários eventos de uma vez (cada um cabe
/// em `16 + NAME_MAX + 1` bytes, e o sistema nunca parte um evento).
#[cfg(any(target_os = "linux", target_os = "android"))]
const EVENT_READ_SIZE: usize = 4096;

/// `FileSystemWatcher::ReadEvents`: os eventos prontos (lista vazia quando
/// não há nenhum, o que encerra o laço de leitura do `_listenOnSocket`);
/// lança o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_ReadEvents(id: i64, file_path: i64) -> i64 {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use inotify::*;
        let _ = file_path;
        let mut buf = vec![0u8; EVENT_READ_SIZE];
        let read_list = match read_from_socket(id, &mut buf) {
            Ok(n) => n,
            Err(e) => {
                throw_os_error(&e);
                return 0;
            }
        };
        let mut events: Vec<i64> = Vec::new();
        let mut p = 0;
        while p + EVENT_SIZE <= read_list {
            let field = |i: usize| u32::from_ne_bytes(buf[p + i..p + i + 4].try_into().unwrap());
            let (wd, mask, cookie, size) = (field(0) as i32, field(4), field(8), field(12) as usize);
            let name = &buf[p + EVENT_SIZE..(p + EVENT_SIZE + size).min(read_list)];
            p += EVENT_SIZE + size;
            if mask & IN_IGNORED != 0 {
                continue;
            }
            let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
            let event = with_roots(&events, || {
                let text = if size > 0 { allocate_str(&String::from_utf8_lossy(name)) } else { 0 };
                with_roots(&[text], || {
                    dart_fixed_list(&[
                        dart_int(inotify_mask(mask)),
                        dart_int(i64::from(cookie)),
                        text,
                        dart_bool(mask & IN_MOVED_TO != 0),
                        dart_int(i64::from(wd)),
                    ])
                })
            });
            events.push(event);
        }
        with_roots(&events, || dart_fixed_list(&events))
    }
    #[cfg(windows)]
    {
        use notification::*;
        let _ = id;
        let buf = match directory_events(file_path) {
            Ok(b) => b,
            Err(e) => {
                throw_os_error(&e);
                return 0;
            }
        };
        let field = |i: usize| u32::from_ne_bytes(buf[i..i + 4].try_into().unwrap());
        let mut events: Vec<i64> = Vec::new();
        let mut p = 0;
        while p + HEADER <= buf.len() {
            let (next_offset, action, size) = (field(p) as usize, field(p + 4), field(p + 8) as usize);
            let end = (p + HEADER + size).min(buf.len());
            let name: Vec<u16> = buf[p + HEADER..end].chunks_exact(2).map(|b| u16::from_ne_bytes([b[0], b[1]])).collect();
            let mask = match action {
                FILE_ACTION_ADDED => fs_event::CREATE,
                FILE_ACTION_REMOVED => fs_event::DELETE,
                FILE_ACTION_MODIFIED => fs_event::MODIFY_CONTENT,
                FILE_ACTION_RENAMED_OLD_NAME | FILE_ACTION_RENAMED_NEW_NAME => fs_event::MOVER,
                _ => 0,
            };
            let event = with_roots(&events, || {
                let text = allocate_str(&String::from_utf16_lossy(&name));
                with_roots(&[text], || {
                    // O cookie 1 e "é destino" verdadeiro para todos, como
                    // a VM: o Dart junta os pares de mudança de nome.
                    dart_fixed_list(&[dart_int(mask), dart_int(1), text, dart_bool(true), dart_int(file_path)])
                })
            });
            events.push(event);
            if next_offset == 0 {
                break;
            }
            p += next_offset;
        }
        with_roots(&events, || dart_fixed_list(&events))
    }
    #[cfg(target_os = "macos")]
    {
        let _ = id;
        let Some(fd) = fsevents::read_descriptor(file_path) else {
            return dart_fixed_list(&[]);
        };
        // Só os registros inteiros (`FDUtils::AvailableBytes` / tamanho),
        // como a VM: um registro maior que `PIPE_BUF` pode chegar em partes.
        let n = (available_bytes(i64::from(fd)).max(0) as usize) / fsevents::RECORD_SIZE;
        let mut events: Vec<i64> = Vec::new();
        let mut rec = vec![0u8; fsevents::RECORD_SIZE];
        for _ in 0..n {
            let mut read_so_far = 0;
            while read_so_far < rec.len() {
                match read_from_socket(i64::from(fd), &mut rec[read_so_far..]) {
                    Ok(0) => break,
                    Ok(k) => read_so_far += k,
                    Err(e) => {
                        throw_os_error(&e);
                        return 0;
                    }
                }
            }
            if read_so_far < rec.len() {
                break;
            }
            let (mask, name) = fsevents::registry_event(&rec);
            let event = with_roots(&events, || {
                let text = allocate_str(&String::from_utf8_lossy(name));
                with_roots(&[text], || {
                    // Cookie 1 e "é destino" verdadeiro, como a VM.
                    dart_fixed_list(&[dart_int(mask), dart_int(1), text, dart_bool(true), dart_int(file_path)])
                })
            });
            events.push(event);
        }
        with_roots(&events, || dart_fixed_list(&events))
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", windows)))]
    {
        let _ = (id, file_path);
        dart_fixed_list(&[])
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
    pub const RECORD_SIZE: usize = PATH_MAX + 8;

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
        fn CFStringCreateWithCString(a: Ref, s: *const c_char, encoding: u32) -> Ref;
        fn CFArrayCreate(a: Ref, value_list: *const Ref, n: isize, callbacks: Ref) -> Ref;
        fn CFRelease(r: Ref);
        fn CFRetain(r: Ref) -> Ref;
        fn CFRunLoopGetCurrent() -> Ref;
        fn CFRunLoopRun();
        fn CFRunLoopStop(rl: Ref);
        fn CFRunLoopWakeUp(rl: Ref);
        fn CFRunLoopTimerCreate(
            a: Ref,
            when: f64,
            interval: f64,
            flag_bits: u64,
            order: isize,
            cb: extern "C" fn(Ref, *mut c_void),
            contexto: *mut TimerContext,
        ) -> Ref;
        fn CFRunLoopAddTimer(rl: Ref, t: Ref, mode: Ref);
        fn CFAbsoluteTimeGetCurrent() -> f64;
        static kCFRunLoopDefaultMode: Ref;
        static kCFRunLoopCommonModes: Ref;
    }

    unsafe extern "C" {
        fn dlopen(file_path: *const c_char, mode: i32) -> *mut c_void;
        fn dlsym(h: *mut c_void, name: *const c_char) -> *mut c_void;
        fn close(fd: i32) -> i32;
    }

    /// `CFRunLoopTimerContext`.
    #[repr(C)]
    struct TimerContext {
        version: isize,
        info: *mut c_void,
        retain: Ref,
        release: Ref,
        description: Ref,
    }

    /// `FSEventStreamContext`.
    #[repr(C)]
    struct StreamContext {
        version: isize,
        info: *mut c_void,
        retain: Ref,
        release: Option<extern "C" fn(*const c_void)>,
        description: Ref,
    }

    type Callback = extern "C" fn(Ref, *mut c_void, usize, *mut c_void, *const u32, *const u64);

    /// As funções do FSEvents (`CoreServices`).
    struct Functions {
        create: unsafe extern "C" fn(Ref, Callback, *mut StreamContext, Ref, u64, f64, u32) -> Ref,
        schedule: unsafe extern "C" fn(Ref, Ref, Ref),
        iniciar: unsafe extern "C" fn(Ref) -> u8,
        drain: unsafe extern "C" fn(Ref),
        stop: unsafe extern "C" fn(Ref),
        invalidate: unsafe extern "C" fn(Ref),
        free: unsafe extern "C" fn(Ref),
    }

    fn functions() -> Result<&'static Functions, super::OsError> {
        static F: std::sync::OnceLock<Option<Functions>> = std::sync::OnceLock::new();
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
                Some(Functions {
                    create: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamCreate")?),
                    schedule: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamScheduleWithRunLoop")?),
                    iniciar: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamStart")?),
                    drain: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamFlushSync")?),
                    stop: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamStop")?),
                    invalidate: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamInvalidate")?),
                    free: std::mem::transmute::<*mut c_void, _>(s(c"FSEventStreamRelease")?),
                })
            }
        })
        .as_ref()
        .ok_or_else(super::OsError::invalid_argument)
    }

    /// O `FSEventsWatcher`: a thread do `CFRunLoop` onde os fluxos rodam.
    pub struct Watcher {
        run_loop: usize,
        thread_handle: Option<std::thread::JoinHandle<()>>,
    }

    extern "C" fn nothing(_t: Ref, _i: *mut c_void) {}

    extern "C" fn stop_loop(_t: Ref, info: *mut c_void) {
        // SAFETY: `info` é o `CFRunLoop` do observador, retido por ele.
        unsafe { CFRunLoopStop(info as Ref) };
    }

    impl Watcher {
        /// Sobe a thread e espera o laço dela existir.
        pub fn iniciar() -> Result<Box<Watcher>, super::OsError> {
            let (tx, rx) = std::sync::mpsc::channel::<usize>();
            let thread_handle = std::thread::Builder::new()
                .name("dart:io FileWatcher".into())
                .spawn(move || {
                    // SAFETY: o laço desta thread; o temporizador vazio o
                    // mantém vivo (sem fontes o `CFRunLoopRun` volta logo).
                    unsafe {
                        let run_loop = CFRunLoopGetCurrent();
                        CFRetain(run_loop);
                        let t = CFRunLoopTimerCreate(
                            std::ptr::null(),
                            CFAbsoluteTimeGetCurrent() + 1.0,
                            1.0,
                            0,
                            0,
                            nothing,
                            std::ptr::null_mut(),
                        );
                        CFRunLoopAddTimer(run_loop, t, kCFRunLoopCommonModes);
                        CFRelease(t);
                        let _ = tx.send(run_loop as usize);
                        CFRunLoopRun();
                    }
                })
                .map_err(|e| super::OsError::de(&e))?;
            let run_loop = rx.recv().map_err(|_| super::OsError::invalid_argument())?;
            Ok(Box::new(Watcher { run_loop, thread_handle: Some(thread_handle) }))
        }

        /// `AddPath`: o pipe, o caminho real e o fluxo; devolve o id do
        /// caminho (o nó).
        pub fn watch_path(&self, file_path: &[u8], recursive: bool) -> Result<i64, super::OsError> {
            use std::os::unix::ffi::OsStrExt;
            let f = functions()?;
            let [read_side, write_side] = super::cloexec_pipe()?;
            super::set_nonblocking(read_side);
            // O caminho real (`realpath`): o FSEvents entrega os caminhos já
            // resolvidos (`/private/var/…` para `/var/…`).
            let raw = std::ffi::OsStr::from_bytes(file_path);
            let real = std::fs::canonicalize(raw).map(|p| p.as_os_str().as_bytes().to_vec()).unwrap_or_else(|_| file_path.to_vec());
            let c = std::ffi::CString::new(real.clone()).map_err(|_| super::OsError::invalid_argument())?;
            // SAFETY: `c` é um texto C válido.
            let text = unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8) };
            let no = Box::into_raw(Box::new(No { base: real.len(), text, read_side, write_side, recursive, stream: std::ptr::null() }));
            DESCRIPTORS.lock().unwrap_or_else(|e| e.into_inner()).insert(no as i64, read_side);
            let mut contexto =
                StreamContext { version: 0, info: no.cast(), retain: std::ptr::null(), release: Some(free_node), description: std::ptr::null() };
            // SAFETY: as funções do FSEvents com os tipos documentados; o nó
            // vive até a `release` do contexto (o fluxo liberado).
            unsafe {
                let list = CFArrayCreate(std::ptr::null(), &(*no).text, 1, std::ptr::null());
                let stream = (f.create)(
                    std::ptr::null(),
                    on_event,
                    &mut contexto,
                    list,
                    K_FS_EVENT_STREAM_EVENT_ID_SINCE_NOW,
                    0.10,
                    K_FS_EVENT_STREAM_CREATE_FLAG_FILE_EVENTS,
                );
                CFRelease(list);
                (*no).stream = stream;
                (f.schedule)(stream, self.run_loop as Ref, kCFRunLoopDefaultMode);
                (f.iniciar)(stream);
                (f.drain)(stream);
            }
            Ok(no as i64)
        }
    }

    impl Drop for Watcher {
        /// `Stop`: agenda a parada no próprio laço e espera a thread.
        fn drop(&mut self) {
            let run_loop = self.run_loop as Ref;
            let mut contexto =
                TimerContext { version: 0, info: run_loop as *mut c_void, retain: std::ptr::null(), release: std::ptr::null(), description: std::ptr::null() };
            // SAFETY: o laço está retido até aqui; o temporizador copia o
            // contexto.
            unsafe {
                let t = CFRunLoopTimerCreate(std::ptr::null(), 0.0, 0.0, 0, 0, stop_loop, &mut contexto);
                CFRunLoopAddTimer(run_loop, t, kCFRunLoopCommonModes);
                CFRelease(t);
                CFRunLoopWakeUp(run_loop);
            }
            if let Some(f) = self.thread_handle.take() {
                let _ = f.join();
            }
            // SAFETY: a retenção feita pela thread.
            unsafe { CFRelease(run_loop) };
        }
    }

    /// Um caminho observado (`FSEventsWatcher::Node`).
    struct No {
        /// O tamanho do caminho real (o prefixo tirado de cada evento).
        base: usize,
        text: Ref,
        read_side: i32,
        write_side: i32,
        recursive: bool,
        stream: Ref,
    }

    /// O lado de leitura do pipe de cada caminho vivo, pelo id: o Dart pode
    /// pedir eventos depois do `UnwatchPath`, e o nó já não existe.
    static DESCRIPTORS: std::sync::Mutex<std::collections::BTreeMap<i64, i32>> =
        std::sync::Mutex::new(std::collections::BTreeMap::new());

    /// O descritor de leitura do caminho `id`, se ele ainda é observado.
    pub fn read_descriptor(id: i64) -> Option<i32> {
        DESCRIPTORS.lock().unwrap_or_else(|e| e.into_inner()).get(&id).copied()
    }

    /// `UnwatchPath`: para e libera o fluxo (a `release` do contexto libera o
    /// nó e fecha o lado de escrita do pipe).
    pub fn stop(id: i64) {
        if DESCRIPTORS.lock().unwrap_or_else(|e| e.into_inner()).remove(&id).is_none() {
            return;
        }
        let Ok(f) = functions() else { return };
        // SAFETY: o nó está vivo até a liberação do fluxo, a última chamada.
        unsafe {
            let stream = (*(id as *const No)).stream;
            (f.stop)(stream);
            (f.invalidate)(stream);
            (f.free)(stream);
        }
    }

    extern "C" fn free_node(info: *const c_void) {
        // SAFETY: `info` é o `Box` do `watch_path`, liberado uma vez (quando o
        // fluxo deixa de existir).
        let no = unsafe { Box::from_raw(info as *mut No) };
        // SAFETY: o lado de escrita é só deste nó; o texto foi criado nele.
        unsafe {
            close(no.write_side);
            CFRelease(no.text);
        }
    }

    /// A *callback* do fluxo, na thread do observador: cada evento vira um
    /// registro no pipe (o caminho relativo ao observado; fora do modo
    /// recursivo, só os do próprio diretório).
    extern "C" fn on_event(_stream: Ref, info: *mut c_void, n: usize, paths: *mut c_void, flag_bits: *const u32, _ids: *const u64) {
        use std::os::unix::ffi::OsStrExt;
        // SAFETY: `info` é o nó vivo (o fluxo o mantém); `paths` e
        // `flag_bits` têm `n` itens.
        let no = unsafe { &*(info as *const No) };
        let paths = paths as *const *const c_char;
        for i in 0..n {
            // SAFETY: ver acima.
            let (c, flags) = unsafe { (std::ffi::CStr::from_ptr(*paths.add(i)).to_bytes(), *flag_bits.add(i)) };
            let exists = std::fs::symlink_metadata(std::ffi::OsStr::from_bytes(c)).is_ok();
            let mut rest = &c[no.base.min(c.len())..];
            if !rest.is_empty() {
                rest = &rest[1..];
            }
            if !no.recursive && rest.contains(&b'/') {
                continue;
            }
            let mut rec = [0u8; RECORD_SIZE];
            rec[..4].copy_from_slice(&u32::from(exists).to_ne_bytes());
            rec[4..8].copy_from_slice(&flags.to_ne_bytes());
            let k = rest.len().min(PATH_MAX - 1);
            rec[8..8 + k].copy_from_slice(&rest[..k]);
            super::write_blocking(no.write_side, &rec);
        }
    }

    /// A máscara de `FileSystemEvent` e o nome de um registro
    /// (`FileSystemWatcher::ReadEvents` da VM).
    pub fn registry_event(r: &[u8]) -> (i64, &[u8]) {
        use super::fs_event::*;
        let exists = u32::from_ne_bytes(r[..4].try_into().unwrap()) != 0;
        let flags = u32::from_ne_bytes(r[4..8].try_into().unwrap());
        let file_path = &r[8..];
        let name = &file_path[..file_path.iter().position(|&b| b == 0).unwrap_or(file_path.len())];
        let mut mask = 0;
        if flags & ITEM_RENAMED != 0 {
            mask |= if name.is_empty() {
                DELETE_SELF
            } else if exists {
                CREATE
            } else {
                DELETE
            };
        }
        if flags & ITEM_MODIFIED != 0 {
            mask |= MODIFY_CONTENT;
        }
        if flags & ITEM_XATTR_MOD != 0 {
            mask |= MODIFY_ATTRIBUTES;
        }
        if flags & ITEM_CREATED != 0 {
            mask |= CREATE;
        }
        if flags & ITEM_IS_DIR != 0 {
            mask |= IS_DIRECTORY;
        }
        if flags & ITEM_REMOVED != 0 {
            mask |= if name.is_empty() { DELETE_SELF } else { DELETE };
        }
        (mask, name)
    }
}
