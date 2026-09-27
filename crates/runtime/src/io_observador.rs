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

/// Os bits de `FileSystemEvent` (e os internos do `_FileSystemWatcher`).
#[cfg(any(target_os = "linux", target_os = "android", windows))]
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
    u8::from(cfg!(any(target_os = "linux", target_os = "android", windows)))
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
    #[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
    {
        lancar_os_error(&ErroDoSo::argumento_invalido());
        0
    }
}

/// `FileSystemWatcher::Close`: o descritor é do soquete interno (ver o
/// cabeçalho); nada a liberar aqui.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_CloseWatcher(_id: i64) {}

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
    #[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
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
    #[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
    let _ = (id, caminho);
}

/// `FileSystemWatcher::GetSocketId`: no Linux, o próprio descritor; no
/// Windows, o manipulador do caminho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_FileSystemWatcher_GetSocketId(id: i64, caminho: i64) -> i64 {
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
    #[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
    {
        let _ = (id, caminho);
        dart_lista_fixa(&[])
    }
}
