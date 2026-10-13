// Runtime nativo: o manipulador de eventos do `dart:io` no Windows
// (`runtime/bin/eventhandler_win.cc` da VM), sobre uma porta de conclusão
// de E/S (IOCP).
//
// No Windows o "descritor" de um `_NativeSocket` é um [`Handle`] (o
// `Handle` da VM): o `SOCKET`/`HANDLE` do sistema, as fichas e portas Dart
// ([`DescriptorInfo`], o mesmo do Unix) e a E/S sobreposta em andamento.
// Não há prontidão para vigiar como no epoll: o manipulador mantém uma
// leitura emitida (`WSARecv`/`ReadFile`/`AcceptEx`) e guarda o que chegou;
// o Dart lê desse buffer (`Socket_Read`), e cada leitura que o esvazia
// emite a seguinte. A escrita copia os bytes e emite `WSASend`/`WriteFile`;
// enquanto ela não conclui o soquete está "cheio" (escreve 0). As
// conclusões e os comandos do Dart chegam pela mesma porta de conclusão e
// são atendidos por uma thread só, como na VM.
//
// A entrada padrão não aceita E/S sobreposta: uma thread faz o `ReadFile`
// síncrono e posta a conclusão na porta.
//
// Tudo o que é do Windows fica neste módulo; a raiz do runtime vê só os
// nomes que os natives comuns usam.

#[cfg(windows)]
mod io_windows {
    use super::*;
    use std::ffi::c_void;
    use std::sync::Arc;

    /// Um `HANDLE` ou `SOCKET` do sistema.
    pub(super) type Bruto = usize;
    /// `INVALID_HANDLE_VALUE` e `INVALID_SOCKET`.
    pub(super) const INVALID: Bruto = usize::MAX;

    pub(super) const ERROR_BROKEN_PIPE: u32 = 109;
    pub(super) const ERROR_IO_PENDING: u32 = 997;
    pub(super) const ERROR_IO_INCOMPLETE: u32 = 996;
    const ERROR_OPERATION_ABORTED: u32 = 995;
    const ERROR_NETNAME_DELETED: u32 = 64;
    const ERROR_CONNECTION_ABORTED: u32 = 1236;
    const ERROR_INVALID_USER_BUFFER: i32 = 1784;
    pub(super) const WSAECONNRESET: u32 = 10054;
    const FILE_TYPE_CHAR: u32 = 2;
    const INFINITE: u32 = u32::MAX;

    pub(super) const AF_INET: u16 = 2;
    pub(super) const AF_INET6: u16 = 23;
    pub(super) const SOCK_STREAM: i32 = 1;
    pub(super) const SOCK_DGRAM: i32 = 2;
    pub(super) const IPPROTO_IP: i32 = 0;
    pub(super) const IPPROTO_TCP: i32 = 6;
    pub(super) const IPPROTO_UDP: i32 = 17;
    pub(super) const IPPROTO_IPV6: i32 = 41;
    pub(super) const SOL_SOCKET: i32 = 0xffff;
    const SO_UPDATE_ACCEPT_CONTEXT: i32 = 0x700b;
    const SO_UPDATE_CONNECT_CONTEXT: i32 = 0x7010;
    const SD_RECEIVE: i32 = 0;
    const SD_SEND: i32 = 1;
    const SD_BOTH: i32 = 2;
    const WSA_FLAG_OVERLAPPED: u32 = 0x1;
    const WSA_FLAG_NO_HANDLE_INHERIT: u32 = 0x80;
    const SIO_GET_EXTENSION_FUNCTION_POINTER: u32 = 0xc800_0006;
    const TF_REUSE_SOCKET: u32 = 0x2;

    /// O tamanho do buffer de leitura (`kBufferSize`), que comporta um
    /// datagrama UDP inteiro.
    const BUFFER_SIZE: usize = 64 * 1024;
    /// O que um `ReadFile` de console lê por vez (`kStdOverlappedBufferSize`).
    const CONSOLE_READ_SIZE: usize = 16 * 1024;
    /// O maior datagrama UDP (`kMaxUDPPackageLength`).
    const MAX_DATAGRAM_SIZE: usize = 64 * 1024;
    /// O espaço de cada endereço do `AcceptEx`: um `SOCKADDR_STORAGE` e
    /// mais 16 bytes (`kAcceptExAddressStorageSize`).
    const ACCEPT_ADDRESS_SIZE: u32 = 128 + 16;
    /// Os `AcceptEx` que um soquete de escuta mantém emitidos
    /// (`kMinIssuedAccepts`).
    const ACCEPTS_ISSUED: usize = 5;

    /// A chave de conclusão dos comandos do Dart (o `InterruptMessage`); a
    /// da E/S é [`IO_KEY`].
    const COMMAND_KEY: usize = 0;
    const IO_KEY: usize = 1;

    /// `OVERLAPPED`.
    #[repr(C)]
    #[derive(Default)]
    pub(super) struct OverlappedOp {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        event: usize,
    }

    impl OverlappedOp {
        /// Um `OVERLAPPED` com o evento dado.
        pub(super) fn with_event(event: usize) -> OverlappedOp {
            OverlappedOp { event, ..OverlappedOp::default() }
        }
    }

    /// `WSABUF`.
    #[repr(C)]
    struct BufferWsa {
        size: u32,
        data: *mut u8,
    }

    /// `GUID`.
    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateIoCompletionPort(file: Bruto, existing: usize, key: usize, threads: u32) -> usize;
        fn GetQueuedCompletionStatus(port: usize, bytes: *mut u32, key: *mut usize, ov: *mut *mut OverlappedOp, wait_entry: u32) -> i32;
        fn PostQueuedCompletionStatus(port: usize, bytes: u32, key: usize, ov: *mut OverlappedOp) -> i32;
        pub(super) fn CloseHandle(h: Bruto) -> i32;
        pub(super) fn ReadFile(h: Bruto, b: *mut u8, n: u32, read_list: *mut u32, ov: *mut OverlappedOp) -> i32;
        pub(super) fn WriteFile(h: Bruto, b: *const u8, n: u32, written: *mut u32, ov: *mut OverlappedOp) -> i32;
        fn CancelIoEx(h: Bruto, ov: *mut OverlappedOp) -> i32;
        pub(super) fn GetLastError() -> u32;
        pub(super) fn GetFileType(h: Bruto) -> u32;
        fn GetStdHandle(which: u32) -> Bruto;
        fn ReadDirectoryChangesW(
            directory: Bruto,
            b: *mut u8,
            n: u32,
            subtree: i32,
            filter_obj: u32,
            read_list: *mut u32,
            ov: *mut OverlappedOp,
            routine: usize,
        ) -> i32;
    }

    #[link(name = "ws2_32")]
    unsafe extern "system" {
        fn WSAStartup(version: u16, data: *mut u8) -> i32;
        pub(super) fn WSAGetLastError() -> i32;
        pub(super) fn WSASetLastError(e: i32);
        fn WSASocketW(family: i32, tipo: i32, protocol: i32, info: *const c_void, group: u32, flag_bits: u32) -> Bruto;
        pub(super) fn closesocket(s: Bruto) -> i32;
        fn shutdown(s: Bruto, como: i32) -> i32;
        pub(super) fn bind(s: Bruto, e: *const u8, n: i32) -> i32;
        pub(super) fn listen(s: Bruto, queue: i32) -> i32;
        pub(super) fn setsockopt(s: Bruto, level: i32, option: i32, v: *const u8, n: i32) -> i32;
        pub(super) fn getsockopt(s: Bruto, level: i32, option: i32, v: *mut u8, n: *mut i32) -> i32;
        pub(super) fn getsockname(s: Bruto, e: *mut u8, n: *mut i32) -> i32;
        pub(super) fn getpeername(s: Bruto, e: *mut u8, n: *mut i32) -> i32;
        fn WSARecv(s: Bruto, b: *mut BufferWsa, n: u32, received_list: *mut u32, flag_bits: *mut u32, ov: *mut OverlappedOp, routine: usize) -> i32;
        fn WSASend(s: Bruto, b: *mut BufferWsa, n: u32, sent: *mut u32, flag_bits: u32, ov: *mut OverlappedOp, routine: usize) -> i32;
        fn WSARecvFrom(
            s: Bruto,
            b: *mut BufferWsa,
            n: u32,
            received_list: *mut u32,
            flag_bits: *mut u32,
            de: *mut u8,
            from_n: *mut i32,
            ov: *mut OverlappedOp,
            routine: usize,
        ) -> i32;
        fn WSASendTo(s: Bruto, b: *mut BufferWsa, n: u32, sent: *mut u32, flag_bits: u32, to: *const u8, para_n: i32, ov: *mut OverlappedOp, routine: usize) -> i32;
        fn WSAIoctl(s: Bruto, code: u32, ent: *const c_void, ent_n: u32, sai: *mut c_void, sai_n: u32, bytes: *mut u32, ov: *mut OverlappedOp, routine: usize) -> i32;
    }

    // -----------------------------------------------------------------------
    // Winsock e as extensões (`AcceptEx`, `ConnectEx`…).

    /// `SocketBase::Initialize`: o `WSAStartup`, uma vez por processo.
    pub(super) fn start_winsock() {
        static STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        STARTED.get_or_init(|| {
            let mut data = [0u8; 512];
            // SAFETY: o `WSADATA` cabe em 512 bytes.
            if unsafe { WSAStartup(0x0202, data.as_mut_ptr()) } != 0 {
                panic!("dart:io: falha ao iniciar o Winsock ({})", unsafe { WSAGetLastError() });
            }
        });
    }

    type FnAcceptEx = unsafe extern "system" fn(Bruto, Bruto, *mut u8, u32, u32, u32, *mut u32, *mut OverlappedOp) -> i32;
    type FnConnectEx = unsafe extern "system" fn(Bruto, *const u8, i32, *const u8, u32, *mut u32, *mut OverlappedOp) -> i32;
    type FnDisconnectEx = unsafe extern "system" fn(Bruto, *mut OverlappedOp, u32, u32) -> i32;
    type FnGetAcceptExSockaddrs = unsafe extern "system" fn(*mut u8, u32, u32, u32, *mut *mut u8, *mut i32, *mut *mut u8, *mut i32);

    /// As funções de extensão do Winsock (`InitializeSocketExtensions`).
    struct Extensions {
        accept_conn: FnAcceptEx,
        connect_socket: FnConnectEx,
        disconnect: FnDisconnectEx,
        accept_addresses: FnGetAcceptExSockaddrs,
    }

    fn extensions() -> &'static Extensions {
        static E: std::sync::OnceLock<Extensions> = std::sync::OnceLock::new();
        E.get_or_init(|| {
            start_winsock();
            // SAFETY: um soquete provisório só para consultar as extensões.
            let s = unsafe { WSASocketW(i32::from(AF_INET), SOCK_STREAM, IPPROTO_TCP, std::ptr::null(), 0, WSA_FLAG_OVERLAPPED) };
            if s == INVALID {
                panic!("dart:io: falha ao criar o soquete das extensões do Winsock");
            }
            let function = |guid: Guid| -> usize {
                let mut f = 0usize;
                let mut n = 0u32;
                // SAFETY: consulta o ponteiro da extensão `guid`.
                let r = unsafe {
                    WSAIoctl(
                        s,
                        SIO_GET_EXTENSION_FUNCTION_POINTER,
                        (&guid as *const Guid).cast(),
                        std::mem::size_of::<Guid>() as u32,
                        (&mut f as *mut usize).cast(),
                        std::mem::size_of::<usize>() as u32,
                        &mut n,
                        std::ptr::null_mut(),
                        0,
                    )
                };
                if r != 0 || f == 0 {
                    panic!("dart:io: extensão do Winsock indisponível");
                }
                f
            };
            // SAFETY: os ponteiros vieram do `WSAIoctl` com os GUIDs
            // (`WSAID_*`) de cada função, com estas assinaturas.
            let e = unsafe {
                Extensions {
                    accept_conn: std::mem::transmute::<usize, FnAcceptEx>(function(Guid(
                        0xb536_7df1,
                        0xcbac,
                        0x11cf,
                        [0x95, 0xca, 0x00, 0x80, 0x5f, 0x48, 0xa1, 0x92],
                    ))),
                    connect_socket: std::mem::transmute::<usize, FnConnectEx>(function(Guid(
                        0x25a2_07b9,
                        0xddf3,
                        0x4660,
                        [0x8e, 0xe9, 0x76, 0xe5, 0x8c, 0x74, 0x06, 0x3e],
                    ))),
                    disconnect: std::mem::transmute::<usize, FnDisconnectEx>(function(Guid(
                        0x7fda_2e11,
                        0x8630,
                        0x436f,
                        [0xa0, 0x31, 0xf5, 0x36, 0xa6, 0xee, 0xc1, 0x57],
                    ))),
                    accept_addresses: std::mem::transmute::<usize, FnGetAcceptExSockaddrs>(function(Guid(
                        0xb536_7df2,
                        0xcbac,
                        0x11cf,
                        [0x95, 0xca, 0x00, 0x80, 0x5f, 0x48, 0xa1, 0x92],
                    ))),
                }
            };
            // SAFETY: o soquete provisório não é mais usado.
            unsafe { closesocket(s) };
            e
        })
    }

    /// Um soquete sobreposto que os filhos não herdam.
    pub(super) fn new_socket(family: u16, tipo: i32, protocol: i32) -> IoResult<Bruto> {
        start_winsock();
        // SAFETY: cria um soquete.
        let s = unsafe {
            WSASocketW(i32::from(family), tipo, protocol, std::ptr::null(), 0, WSA_FLAG_OVERLAPPED | WSA_FLAG_NO_HANDLE_INHERIT)
        };
        if s == INVALID {
            return Err(OsError::from_code(unsafe { WSAGetLastError() }));
        }
        Ok(s)
    }

    // -----------------------------------------------------------------------
    // As operações sobrepostas (`OverlappedBuffer`).

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum OperationKind {
        Accept,
        Read,
        Receive,
        Write,
        SendOp,
        Disconnect,
        Connection,
    }

    /// Uma operação em andamento: o `OVERLAPPED` (primeiro campo, para a
    /// conclusão achar a operação), o buffer e a referência do
    /// [`Handle`] que ela mantém viva até concluir.
    #[repr(C)]
    struct Operation {
        overlapped: OverlappedOp,
        tipo: OperationKind,
        handle: Option<Arc<Handle>>,
        data: Vec<u8>,
        /// O próximo byte a ler e quantos há (leituras).
        ix: usize,
        size: usize,
        buffer_wsa: BufferWsa,
        flag_bits: u32,
        /// O endereço de origem (`WSARecvFrom`) ou de destino (`WSASendTo`).
        address: [u8; 128],
        address_size: i32,
        /// O soquete que o `AcceptEx` preenche.
        client: Bruto,
    }

    impl Operation {
        fn new_area(tipo: OperationKind, m: &Arc<Handle>, capacidade: usize) -> Box<Operation> {
            Box::new(Operation {
                overlapped: OverlappedOp::default(),
                tipo,
                handle: Some(Arc::clone(m)),
                data: vec![0; capacidade],
                ix: 0,
                size: 0,
                buffer_wsa: BufferWsa { size: 0, data: std::ptr::null_mut() },
                flag_bits: 0,
                address: [0; 128],
                address_size: 128,
                client: INVALID,
            })
        }

        fn clean_overlapped(&mut self) -> *mut OverlappedOp {
            self.overlapped = OverlappedOp::default();
            &mut self.overlapped
        }

        fn buffer_wsa(&mut self) -> *mut BufferWsa {
            self.buffer_wsa = BufferWsa { size: self.data.len() as u32, data: self.data.as_mut_ptr() };
            &mut self.buffer_wsa
        }

        fn remaining(&self) -> usize {
            self.size - self.ix
        }

        fn do_read(&mut self, dest: &mut [u8]) -> usize {
            let n = dest.len().min(self.remaining());
            dest[..n].copy_from_slice(&self.data[self.ix..self.ix + n]);
            self.ix += n;
            n
        }

        /// Entrega a operação ao sistema, que a devolve na conclusão.
        fn issued(self: Box<Self>) {
            let _ = Box::into_raw(self);
        }
    }

    // SAFETY: o único ponteiro da operação (`buffer_wsa.data`) aponta para
    // o próprio `data`; a operação passa entre threads inteira (a do Dart,
    // o sistema e a do manipulador), nunca compartilhada.
    unsafe impl Send for Operation {}

    impl Drop for Operation {
        fn drop(&mut self) {
            if self.client != INVALID {
                // SAFETY: o soquete do `AcceptEx` que ninguém adotou.
                unsafe { closesocket(self.client) };
            }
        }
    }

    // -----------------------------------------------------------------------
    // O `Handle`.

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum HandleKind {
        /// Um arquivo ou pipe sobreposto (os pipes dos processos e dos
        /// sinais).
        File,
        /// A entrada padrão, sem E/S sobreposta.
        Pattern,
        Client,
        Listen,
        Datagram,
        /// Um diretório observado (`DirectoryWatchHandle`): a leitura é o
        /// `ReadDirectoryChangesW`.
        Directory,
    }

    /// O `Handle` da VM.
    pub(super) struct Handle {
        pub(super) tipo: HandleKind,
        pub(super) raw: Bruto,
        overlapped: bool,
        /// Diretório observado: o filtro do `ReadDirectoryChangesW`
        /// (`FILE_NOTIFY_CHANGE_*`) e se a subárvore entra.
        watch: (u32, bool),
        estado: std::sync::Mutex<State>,
        /// Acorda a thread de leitura síncrona (entrada padrão).
        read_request: std::sync::Condvar,
    }

    pub(super) struct State {
        /// `kClosing`, `kCloseRead`, `kCloseWrite`, `kError`.
        closing: bool,
        read_closed: bool,
        write_closed: bool,
        error: bool,
        /// O objeto do sistema ainda precisa ser fechado.
        is_open: bool,
        info: DescriptorInfo,
        /// Os dados lidos que o Dart ainda não consumiu (`data_ready_`).
        ready_flag: Option<Box<Operation>>,
        pending_read: bool,
        pending_write: bool,
        pub(super) last_error: u32,
        // Cliente.
        connected: bool,
        disconnected: bool,
        pub(super) remote: Option<OsAddress>,
        // Escuta.
        pub(super) family: u16,
        pending_accepts: usize,
        accepted_list: std::collections::VecDeque<Arc<Handle>>,
        // Entrada padrão: a leitura entregue à thread e se ela existe.
        sync_read: Option<Box<Operation>>,
        reader_thread: bool,
    }

    impl State {
        /// O handle passa a quem chama (`Process::Wait`): o manipulador não
        /// o fecha nem faz mais E/S nele.
        pub(super) fn take_handle(&mut self) {
            self.closing = true;
            self.is_open = false;
        }
    }

    impl Handle {
        /// Cria o manipulador de `raw`, associado à porta de conclusão
        /// quando faz E/S sobreposta.
        pub(super) fn new(tipo: HandleKind, raw: Bruto) -> Arc<Handle> {
            Handle::new_with(tipo, raw, (0, false))
        }

        fn new_with(tipo: HandleKind, raw: Bruto, watch: (u32, bool)) -> Arc<Handle> {
            let overlapped = tipo != HandleKind::Pattern;
            let m = Arc::new(Handle {
                tipo,
                raw,
                overlapped,
                watch,
                estado: std::sync::Mutex::new(State {
                    closing: false,
                    read_closed: false,
                    write_closed: false,
                    error: false,
                    is_open: true,
                    info: DescriptorInfo::new(tipo == HandleKind::Listen),
                    ready_flag: None,
                    pending_read: false,
                    pending_write: false,
                    last_error: 0,
                    connected: false,
                    disconnected: false,
                    remote: None,
                    family: AF_INET,
                    pending_accepts: 0,
                    accepted_list: std::collections::VecDeque::new(),
                    sync_read: None,
                    reader_thread: false,
                }),
                read_request: std::sync::Condvar::new(),
            });
            if overlapped {
                let port = event_handler().port;
                // SAFETY: associa um handle aberto à porta de conclusão.
                if unsafe { CreateIoCompletionPort(raw, port, IO_KEY, 0) } == 0 {
                    panic!("dart:io: falha ao associar o handle à porta de conclusão ({})", unsafe { GetLastError() });
                }
            }
            m
        }

        pub(super) fn estado(&self) -> std::sync::MutexGuard<'_, State> {
            self.estado.lock().unwrap_or_else(|e| e.into_inner())
        }

        fn is_socket(&self) -> bool {
            matches!(self.tipo, HandleKind::Client | HandleKind::Listen | HandleKind::Datagram)
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            let e = self.estado.get_mut().unwrap_or_else(|e| e.into_inner());
            if e.is_open {
                // SAFETY: o objeto do sistema é deste manipulador, que não
                // o fechou.
                unsafe {
                    if self.is_socket() {
                        closesocket(self.raw);
                    } else {
                        CloseHandle(self.raw);
                    }
                }
            }
        }
    }

    /// O descritor Dart de um manipulador (a referência passa ao objeto).
    pub(super) fn descriptor_of(m: Arc<Handle>) -> i64 {
        Arc::into_raw(m) as i64
    }

    /// O manipulador de um descritor, com uma referência nova.
    ///
    /// # Safety
    /// `d` veio de [`descriptor_of`] e a referência dele ainda existe.
    pub(super) unsafe fn retained_handle(d: i64) -> Arc<Handle> {
        let p = d as *const Handle;
        // SAFETY: garantido por quem chama.
        unsafe {
            Arc::increment_strong_count(p);
            Arc::from_raw(p)
        }
    }

    /// `Socket::CloseFd` do Windows: solta a referência do objeto.
    pub(super) fn close_socket_descriptor(d: i64) {
        // SAFETY: a referência do objeto, solta uma vez (o descritor vira
        // -1 antes).
        drop(unsafe { Arc::from_raw(d as *const Handle) });
    }

    pub(super) fn retain_descriptor(d: i64) -> i64 {
        // SAFETY: `d` é o descritor vivo de um soquete de escuta registrado.
        std::mem::forget(unsafe { retained_handle(d) });
        d
    }

    /// O manipulador de um descritor que o objeto Dart mantém vivo durante
    /// o native (`None` se o objeto já o soltou: -1).
    pub(super) fn handle_of(d: i64) -> Option<Arc<Handle>> {
        // SAFETY: um descritor não negativo é uma referência viva do objeto.
        (d >= 0).then(|| unsafe { retained_handle(d) })
    }

    /// O manipulador do soquete de um objeto Dart (`None` se fechado).
    pub(super) fn socket_handle(s: &NativeSocket) -> Option<Arc<Handle>> {
        handle_of(s.descriptor())
    }

    /// `ERROR_INVALID_HANDLE`: o soquete já foi fechado.
    pub(super) fn invalid_handle() -> OsError {
        OsError::from_code(6)
    }

    // -----------------------------------------------------------------------
    // Eventos para o Dart.

    const IN_MASK: i64 = 1 << EVENT_IN;
    const OUT_MASK: i64 = 1 << EVENT_OUT;

    /// `DispatchEventIfEnabled`.
    fn dispatch_events(e: &mut State, event: i64) -> bool {
        if e.info.mask() & event != 0 {
            let p = e.info.next_port();
            post_event(p, event);
            return true;
        }
        false
    }

    /// `HandleClosed`.
    fn notify_close(e: &mut State) {
        if !e.closing {
            e.info.notify_all_waiters(1 << EVENT_CLOSE);
        }
    }

    /// `HandleError`.
    fn notify_error(e: &mut State, code: u32) {
        e.last_error = code;
        e.error = true;
        if !e.closing {
            e.info.notify_all_waiters(1 << EVENT_ERROR);
        }
    }

    /// `HandleIssueError`: o fim do pipe ou a conexão desfeita é
    /// fechamento; o resto é erro.
    fn issue_failure(m: &Handle, e: &mut State, code: u32) {
        let end = if m.is_socket() { WSAECONNRESET } else { ERROR_BROKEN_PIPE };
        if code == end {
            notify_close(e);
        } else {
            notify_error(e, code);
        }
    }

    /// `IsClosed` de cada tipo.
    fn is_closed(m: &Handle, e: &State) -> bool {
        match m.tipo {
            HandleKind::Listen => e.closing && e.pending_accepts == 0,
            HandleKind::Client => e.connected && e.disconnected && !e.pending_read && !e.pending_write,
            _ => e.closing && !e.pending_read && !e.pending_write,
        }
    }

    /// `NotifyDestroyedIfClosed`.
    fn notify_destroy_if_closed(m: &Handle, e: &mut State) {
        if is_closed(m, e) {
            e.info.notify_all_waiters(1 << EVENT_DESTROYED);
            e.info.remove_all();
        }
    }

    // -----------------------------------------------------------------------
    // Emissão de E/S (com o estado travado).

    fn last_system_error() -> u32 {
        // SAFETY: lê o erro da thread.
        unsafe { GetLastError() }
    }

    /// `IssueReadLocked`: mantém uma leitura em andamento enquanto o Dart
    /// não tem dados para consumir.
    fn issue_read(m: &Arc<Handle>, e: &mut State) -> bool {
        match m.tipo {
            HandleKind::Datagram => return issue_receive(m, e),
            HandleKind::Listen => return false,
            _ => {}
        }
        if e.pending_read || e.ready_flag.is_some() || e.closing || e.read_closed {
            return true;
        }
        let mut op = Operation::new_area(OperationKind::Read, m, BUFFER_SIZE);
        e.pending_read = true;
        if !m.overlapped {
            // A thread de leitura faz o `ReadFile` síncrono e posta a
            // conclusão.
            e.sync_read = Some(op);
            if !e.reader_thread {
                e.reader_thread = true;
                let m2 = Arc::clone(m);
                std::thread::Builder::new()
                    .name("dart:io ReadFile".to_string())
                    .spawn(move || sync_read_loop(&m2))
                    .expect("dart:io: falha ao criar a thread de leitura");
            }
            m.read_request.notify_one();
            return true;
        }
        let ok = if m.tipo == HandleKind::Client {
            let b = op.buffer_wsa();
            let f: *mut u32 = &mut op.flag_bits;
            let ov = op.clean_overlapped();
            // SAFETY: a operação (buffer, bandeiras, `OVERLAPPED`) vive
            // até a conclusão.
            unsafe { WSARecv(m.raw, b, 1, std::ptr::null_mut(), f, ov, 0) == 0 }
        } else if m.tipo == HandleKind::Directory {
            // `DirectoryWatchHandle::IssueReadLocked`: os registros
            // `FILE_NOTIFY_INFORMATION` chegam no buffer da operação.
            let (filter_obj, subtree) = m.watch;
            let (p, n) = (op.data.as_mut_ptr(), op.data.len() as u32);
            let ov = op.clean_overlapped();
            // SAFETY: como acima. O `ReadDirectoryChangesW` exige o buffer
            // alinhado a `DWORD`; o do `Vec` vem do `HeapAlloc` (o alocador
            // do sistema no Windows), alinhado a 16.
            unsafe { ReadDirectoryChangesW(m.raw, p, n, i32::from(subtree), filter_obj, std::ptr::null_mut(), ov, 0) != 0 }
        } else {
            let (p, n) = (op.data.as_mut_ptr(), op.data.len() as u32);
            let ov = op.clean_overlapped();
            // SAFETY: como acima.
            unsafe { ReadFile(m.raw, p, n, std::ptr::null_mut(), ov) != 0 }
        };
        let code = last_system_error();
        if ok || code == ERROR_IO_PENDING {
            op.issued();
            return true;
        }
        e.pending_read = false;
        drop(op);
        // O `DirectoryWatchHandle::IssueReadLocked` da VM devolve a falha
        // sem avisar o Dart (um arquivo no lugar do diretório, por exemplo,
        // fica sem eventos); os outros manipuladores avisam.
        if m.tipo != HandleKind::Directory {
            issue_failure(m, e, code);
        }
        false
    }

    /// `DatagramSocket::IssueRecvFromLocked`.
    fn issue_receive(m: &Arc<Handle>, e: &mut State) -> bool {
        if e.pending_read || e.ready_flag.is_some() || e.closing || e.read_closed {
            return true;
        }
        let mut op = Operation::new_area(OperationKind::Receive, m, MAX_DATAGRAM_SIZE);
        let b = op.buffer_wsa();
        let f: *mut u32 = &mut op.flag_bits;
        let de = op.address.as_mut_ptr();
        let from_n: *mut i32 = &mut op.address_size;
        let ov = op.clean_overlapped();
        e.pending_read = true;
        // SAFETY: a operação vive até a conclusão.
        let ok = unsafe { WSARecvFrom(m.raw, b, 1, std::ptr::null_mut(), f, de, from_n, ov, 0) == 0 };
        let code = last_system_error();
        if ok || code == ERROR_IO_PENDING {
            op.issued();
            return true;
        }
        e.pending_read = false;
        drop(op);
        issue_failure(m, e, code);
        false
    }

    /// `ListenSocket::IssueAcceptLocked`.
    fn issue_accept(m: &Arc<Handle>, e: &mut State) -> bool {
        let client = match new_socket(e.family, SOCK_STREAM, IPPROTO_TCP) {
            Ok(s) => s,
            Err(error) => {
                notify_error(e, error.code as u32);
                return false;
            }
        };
        let mut op = Operation::new_area(OperationKind::Accept, m, 2 * ACCEPT_ADDRESS_SIZE as usize);
        op.client = client;
        let p = op.data.as_mut_ptr();
        let ov = op.clean_overlapped();
        let mut received_list = 0u32;
        // SAFETY: a operação (buffer dos endereços, `OVERLAPPED`) vive até
        // a conclusão.
        let ok = unsafe {
            (extensions().accept_conn)(m.raw, client, p, 0, ACCEPT_ADDRESS_SIZE, ACCEPT_ADDRESS_SIZE, &mut received_list, ov) != 0
        };
        let code = last_system_error();
        if ok || code == ERROR_IO_PENDING {
            e.pending_accepts += 1;
            op.issued();
            return true;
        }
        drop(op);
        notify_error(e, code);
        false
    }

    /// O `ConnectEx` de `Socket::CreateConnect`: conclui pela porta, mesmo
    /// quando termina de imediato.
    pub(super) fn start_connect(m: &Arc<Handle>, dest: &OsAddress) -> IoResult<()> {
        let mut op = Operation::new_area(OperationKind::Connection, m, 0);
        let ov = op.clean_overlapped();
        // SAFETY: `dest` é um `sockaddr` válido; a operação vive até a
        // conclusão.
        let ok = unsafe {
            (extensions().connect_socket)(m.raw, dest.bytes.as_ptr(), dest.family_size() as i32, std::ptr::null(), 0, std::ptr::null_mut(), ov) != 0
        };
        let code = last_system_error();
        if ok || code == ERROR_IO_PENDING {
            op.issued();
            return Ok(());
        }
        drop(op);
        Err(OsError::from_code(code as i32))
    }

    /// `ListenSocket::StartAccept`: `false` com o erro em `last_error`.
    pub(super) fn start_accepting(m: &Arc<Handle>) -> bool {
        let mut e = m.estado();
        (0..ACCEPTS_ISSUED).all(|_| issue_accept(m, &mut e))
    }

    /// `ClientSocket::IssueDisconnectLocked`: a desconexão e o aviso de
    /// destruição às portas.
    fn issue_disconnect(m: &Arc<Handle>, e: &mut State) {
        let mut op = Operation::new_area(OperationKind::Disconnect, m, 0);
        let ov = op.clean_overlapped();
        // SAFETY: a operação vive até a conclusão.
        let ok = unsafe { (extensions().disconnect)(m.raw, ov, TF_REUSE_SOCKET, 0) != 0 };
        let code = last_system_error();
        if ok || code == ERROR_IO_PENDING {
            // Mesmo concluída de imediato, a conclusão passa pela porta.
            op.issued();
        } else {
            drop(op);
            finish_disconnect(m, e);
        }
        e.info.notify_all_waiters(1 << EVENT_DESTROYED);
        e.info.remove_all();
    }

    /// `ClientSocket::DisconnectComplete`.
    fn finish_disconnect(m: &Handle, e: &mut State) {
        if e.is_open {
            // SAFETY: o soquete é deste manipulador.
            unsafe { closesocket(m.raw) };
            e.is_open = false;
        }
        e.ready_flag = None;
        e.disconnected = true;
    }

    /// `ClientSocket::ConnectComplete`.
    fn finish_connect(m: &Arc<Handle>, e: &mut State) {
        // SAFETY: atualiza o contexto do soquete conectado pelo
        // `ConnectEx`.
        unsafe { setsockopt(m.raw, SOL_SOCKET, SO_UPDATE_CONNECT_CONTEXT, std::ptr::null(), 0) };
        if !e.read_closed && e.info.mask() & IN_MASK != 0 {
            issue_read(m, e);
        }
        if !e.write_closed {
            dispatch_events(e, OUT_MASK);
        }
    }

    /// `ListenSocket::DispatchCompletedAcceptsLocked`.
    fn dispatch_accepts(e: &mut State) {
        if e.closing {
            return;
        }
        for _ in 0..e.accepted_list.len() {
            if !dispatch_events(e, IN_MASK) {
                break;
            }
        }
    }

    /// `Handle::CloseLocked`.
    fn close_handle(m: &Arc<Handle>, e: &mut State) {
        if !m.overlapped && e.is_open {
            // Destrava o `ReadFile` síncrono da thread de leitura.
            // SAFETY: cancela a E/S do handle.
            unsafe { CancelIoEx(m.raw, std::ptr::null_mut()) };
        }
        if e.closing {
            return;
        }
        e.closing = true;
        match m.tipo {
            HandleKind::File | HandleKind::Pattern | HandleKind::Directory => {
                if e.is_open {
                    // SAFETY: o handle é deste manipulador; a E/S pendente
                    // conclui com `ERROR_OPERATION_ABORTED` (a do diretório
                    // observado também: `DirectoryWatchHandle::Stop`).
                    unsafe { CloseHandle(m.raw) };
                    e.is_open = false;
                }
                if m.tipo == HandleKind::Pattern {
                    m.read_request.notify_all();
                    let mut g = stdin_entry();
                    if g.as_ref().is_some_and(|x| std::ptr::eq(Arc::as_ptr(x), Arc::as_ptr(m))) {
                        *g = None;
                    }
                }
            }
            HandleKind::Listen => {
                if e.is_open {
                    // SAFETY: os `AcceptEx` pendentes concluem abortados.
                    unsafe { closesocket(m.raw) };
                    e.is_open = false;
                }
                for c in std::mem::take(&mut e.accepted_list) {
                    let mut ce = c.estado();
                    close_handle(&c, &mut ce);
                    notify_destroy_if_closed(&c, &mut ce);
                }
            }
            HandleKind::Client => {
                // SAFETY: encerra os dois sentidos antes de desconectar.
                unsafe { shutdown(m.raw, SD_BOTH) };
                issue_disconnect(m, e);
            }
            HandleKind::Datagram => {
                if e.is_open {
                    // SAFETY: a E/S pendente conclui abortada.
                    unsafe { closesocket(m.raw) };
                    e.is_open = false;
                }
                e.read_closed = true;
                e.write_closed = true;
            }
        }
    }

    /// Fecha um manipulador que nenhum objeto Dart chegou a usar.
    pub(super) fn discard(m: &Arc<Handle>) {
        let mut e = m.estado();
        match m.tipo {
            HandleKind::Client => {
                // Nunca conectou: fecha já, sem `DisconnectEx`.
                e.closing = true;
                finish_disconnect(m, &mut e);
            }
            _ => close_handle(m, &mut e),
        }
    }

    /// A thread da leitura síncrona (`Handle::ReadSyncCompleteAsync`):
    /// atende um pedido por vez e posta a conclusão na porta.
    fn sync_read_loop(m: &Arc<Handle>) {
        let port = event_handler().port;
        loop {
            let mut op = {
                let mut e = m.estado();
                loop {
                    if let Some(op) = e.sync_read.take() {
                        break op;
                    }
                    if e.closing {
                        e.reader_thread = false;
                        return;
                    }
                    e = m.read_request.wait(e).unwrap_or_else(|x| x.into_inner());
                }
            };
            // SAFETY: consulta o tipo do handle.
            let n = if unsafe { GetFileType(m.raw) } == FILE_TYPE_CHAR { CONSOLE_READ_SIZE } else { op.data.len() };
            let mut read_list = 0u32;
            // SAFETY: leitura síncrona no buffer da operação.
            if unsafe { ReadFile(m.raw, op.data.as_mut_ptr(), n as u32, &mut read_list, std::ptr::null_mut()) } == 0 {
                read_list = 0;
            }
            let ov = op.clean_overlapped();
            op.issued();
            // SAFETY: a operação volta pela porta, como uma sobreposta.
            if unsafe { PostQueuedCompletionStatus(port, read_list, IO_KEY, ov) } == 0 {
                panic!("dart:io: PostQueuedCompletionStatus falhou ({})", unsafe { GetLastError() });
            }
        }
    }

    // -----------------------------------------------------------------------
    // A entrada padrão.

    fn stdin_entry() -> std::sync::MutexGuard<'static, Option<Arc<Handle>>> {
        static E: std::sync::Mutex<Option<Arc<Handle>>> = std::sync::Mutex::new(None);
        E.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `SocketBase::GetStdioHandle`: só a entrada (`StdHandle::Stdin`, um
    /// por processo).
    pub(super) fn default_handle(num: i64) -> i64 {
        if num != 0 {
            return -1;
        }
        // SAFETY: lê o handle do processo (STD_INPUT_HANDLE = -10).
        let h = unsafe { GetStdHandle(-10i32 as u32) };
        if h == INVALID || h == 0 {
            return -1;
        }
        let mut g = stdin_entry();
        let m = Arc::clone(g.get_or_insert_with(|| Handle::new(HandleKind::Pattern, h)));
        descriptor_of(m)
    }

    // -----------------------------------------------------------------------
    // As operações que os natives fazem (`Handle::Read`, `Write`…).

    /// `Handle::Available`.
    pub(super) fn available(s: &NativeSocket) -> i64 {
        socket_handle(s).map_or(0, |m| m.estado().ready_flag.as_ref().map_or(0, |p| p.remaining() as i64))
    }

    /// `Handle::Read`: o que já chegou; esvaziar o buffer emite a próxima
    /// leitura.
    pub(super) fn read_from(s: &NativeSocket, dest: &mut [u8]) -> IoResult<usize> {
        let Some(m) = socket_handle(s) else { return Ok(0) };
        read_from_handle(&m, dest)
    }

    fn read_from_handle(m: &Arc<Handle>, dest: &mut [u8]) -> IoResult<usize> {
        let mut e = m.estado();
        let Some(p) = e.ready_flag.as_mut() else { return Ok(0) };
        let n = p.do_read(dest);
        if p.remaining() == 0 {
            e.ready_flag = None;
            if !e.closing && !e.read_closed {
                issue_read(m, &mut e);
            }
        }
        Ok(n)
    }

    // -----------------------------------------------------------------------
    // Observação de diretórios (`file_system_watcher_win.cc`).

    /// `FILE_LIST_DIRECTORY`.
    const FILE_LIST_DIRECTORY: u32 = 0x0001;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;

    /// `FileSystemWatcher::WatchPath`: abre o diretório para E/S
    /// sobreposta, cria o manipulador (o `DirectoryWatchHandle`, associado
    /// à porta de conclusão) e já emite a primeira leitura, para os eventos
    /// contarem desde agora (`Start`). O descritor devolvido é o id do
    /// caminho e o do soquete interno que o Dart cria com ele.
    pub(super) fn watch_directory(file_path: &str, filter_obj: u32, recursive: bool) -> IoResult<i64> {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::IntoRawHandle;
        let file = std::fs::OpenOptions::new()
            .access_mode(FILE_LIST_DIRECTORY)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED)
            .open(file_path)
            .map_err(|e| OsError::de(&e))?;
        let m = Handle::new_with(HandleKind::Directory, file.into_raw_handle() as Bruto, (filter_obj, recursive));
        {
            let mut e = m.estado();
            issue_read(&m, &mut e);
        }
        Ok(descriptor_of(m))
    }

    /// `FileSystemWatcher::UnwatchPath` (`DirectoryWatchHandle::Stop`):
    /// cancela a leitura e fecha o handle; a referência continua com o
    /// soquete interno, que o Dart fecha em seguida.
    pub(super) fn stop_watch(d: i64) {
        if let Some(m) = handle_of(d) {
            let mut e = m.estado();
            close_handle(&m, &mut e);
            notify_destroy_if_closed(&m, &mut e);
        }
    }

    /// `FileSystemWatcher::ReadEvents`: os registros que já chegaram
    /// (`Available` e `Read`; esvaziar emite a próxima leitura).
    pub(super) fn directory_events(d: i64) -> IoResult<Vec<u8>> {
        let Some(m) = handle_of(d) else { return Ok(Vec::new()) };
        let available = m.estado().ready_flag.as_ref().map_or(0, |p| p.remaining());
        let mut buf = vec![0u8; available];
        let n = read_from_handle(&m, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    /// `Handle::Write`: copia até 64 KiB e emite a escrita; 0 enquanto
    /// outra está em andamento.
    pub(super) fn write_to_handle(m: &Arc<Handle>, data: &[u8]) -> IoResult<usize> {
        let mut e = m.estado();
        if e.pending_write || is_closed(m, &e) {
            return Ok(0);
        }
        let n = data.len().min(BUFFER_SIZE);
        if !m.overlapped {
            // A entrada padrão não é escrita pelo Dart; um handle síncrono
            // escreve na hora.
            let mut written = 0u32;
            // SAFETY: escrita síncrona dos bytes dados.
            if unsafe { WriteFile(m.raw, data.as_ptr(), n as u32, &mut written, std::ptr::null_mut()) } == 0 {
                return Err(OsError::from_code(last_system_error() as i32));
            }
            return Ok(written as usize);
        }
        let mut op = Operation::new_area(OperationKind::Write, m, 0);
        op.data = data[..n].to_vec();
        op.size = n;
        e.pending_write = true;
        let ok = if m.tipo == HandleKind::Client {
            let b = op.buffer_wsa();
            let ov = op.clean_overlapped();
            // SAFETY: a operação vive até a conclusão.
            unsafe { WSASend(m.raw, b, 1, std::ptr::null_mut(), 0, ov, 0) == 0 }
        } else {
            let p = op.data.as_ptr();
            let ov = op.clean_overlapped();
            // SAFETY: como acima.
            unsafe { WriteFile(m.raw, p, n as u32, std::ptr::null_mut(), ov) != 0 }
        };
        let code = last_system_error();
        if ok || code == ERROR_IO_PENDING {
            op.issued();
            return Ok(n);
        }
        e.pending_write = false;
        drop(op);
        issue_failure(m, &mut e, code);
        Err(OsError::from_code(code as i32))
    }

    pub(super) fn write_at(s: &NativeSocket, data: &[u8]) -> IoResult<usize> {
        match socket_handle(s) {
            Some(m) => write_to_handle(&m, data),
            None => Ok(0),
        }
    }

    /// `SocketBase::HasPendingWrite`.
    pub(super) fn pending_write(s: &NativeSocket) -> bool {
        socket_handle(s).is_some_and(|m| m.estado().pending_write)
    }

    /// `ServerSocket::Accept`: a próxima conexão aceita (o descritor dela)
    /// ou -1; repõe os `AcceptEx` emitidos.
    pub(super) fn accept_conn(fd: i64) -> i64 {
        let Some(m) = handle_of(fd) else { return -1 };
        let mut e = m.estado();
        let r = e.accepted_list.pop_front();
        if !e.closing && e.pending_accepts < ACCEPTS_ISSUED {
            issue_accept(&m, &mut e);
        }
        r.map_or(-1, descriptor_of)
    }

    /// `SocketBase::AvailableDatagram`.
    pub(super) fn has_datagram(fd: i64) -> bool {
        handle_of(fd).is_some_and(|m| m.estado().ready_flag.is_some())
    }

    /// `Handle::RecvFrom`: o datagrama recebido (o buffer inteiro sai, como
    /// o `recvfrom`) e a origem.
    pub(super) fn receive_datagram(fd: i64, maximum: usize) -> IoResult<Option<(Vec<u8>, OsAddress)>> {
        let Some(m) = handle_of(fd) else { return Ok(None) };
        let mut e = m.estado();
        let Some(op) = e.ready_flag.take() else { return Ok(None) };
        let n = op.remaining().min(maximum);
        let data = op.data[op.ix..op.ix + n].to_vec();
        let origin = OsAddress::from_pointer(op.address.as_ptr(), op.address_size.max(0) as u32);
        drop(op);
        if !e.closing && !e.read_closed {
            issue_receive(&m, &mut e);
        }
        Ok(Some((data, origin)))
    }

    /// `Handle::SendTo`.
    pub(super) fn send_datagram(fd: i64, data: &[u8], dest: &OsAddress) -> IoResult<usize> {
        let Some(m) = handle_of(fd) else { return Err(invalid_handle()) };
        let mut e = m.estado();
        if e.pending_write || is_closed(&m, &e) {
            return Ok(0);
        }
        if data.len() > MAX_DATAGRAM_SIZE {
            return Err(OsError::from_code(ERROR_INVALID_USER_BUFFER));
        }
        let mut op = Operation::new_area(OperationKind::SendOp, &m, 0);
        op.data = data.to_vec();
        op.size = data.len();
        let n_dest = dest.family_size() as usize;
        op.address[..n_dest].copy_from_slice(&dest.bytes[..n_dest]);
        op.address_size = n_dest as i32;
        let b = op.buffer_wsa();
        let to = op.address.as_ptr();
        let ov = op.clean_overlapped();
        e.pending_write = true;
        // SAFETY: a operação (dados, endereço, `OVERLAPPED`) vive até a
        // conclusão.
        let ok = unsafe { WSASendTo(m.raw, b, 1, std::ptr::null_mut(), 0, to, n_dest as i32, ov, 0) == 0 };
        let code = last_system_error();
        if ok || code == ERROR_IO_PENDING {
            op.issued();
            return Ok(data.len());
        }
        e.pending_write = false;
        drop(op);
        issue_failure(&m, &mut e, code);
        Err(OsError::from_code(code as i32))
    }

    // -----------------------------------------------------------------------
    // A thread do manipulador.

    pub(super) fn start_handler() -> EventHandler {
        // SAFETY: cria a porta de conclusão (uma thread concorrente).
        let port = unsafe { CreateIoCompletionPort(INVALID, 0, 0, 1) };
        if port == 0 {
            panic!("dart:io: falha ao criar a porta de conclusão ({})", unsafe { GetLastError() });
        }
        std::thread::Builder::new()
            .name("dart:io EventHandler".to_string())
            .spawn(move || event_loop(port))
            .expect("dart:io: falha ao criar a thread do manipulador de eventos");
        EventHandler { port }
    }

    impl EventHandler {
        /// `SendData`: o comando vai pela porta de conclusão.
        pub(super) fn enviar(&self, c: EventCommand) {
            let p = Box::into_raw(Box::new(c));
            // SAFETY: o laço recebe o ponteiro com a chave de comando e o
            // retoma.
            if unsafe { PostQueuedCompletionStatus(self.port, 0, COMMAND_KEY, p.cast()) } == 0 {
                panic!("dart:io: PostQueuedCompletionStatus falhou ({})", unsafe { GetLastError() });
            }
        }
    }

    fn event_loop(port: usize) {
        loop {
            let mut bytes = 0u32;
            let mut key = 0usize;
            let mut ov: *mut OverlappedOp = std::ptr::null_mut();
            // SAFETY: espera uma conclusão na porta.
            let ok = unsafe { GetQueuedCompletionStatus(port, &mut bytes, &mut key, &mut ov, INFINITE) } != 0;
            if ov.is_null() {
                continue;
            }
            if key == COMMAND_KEY {
                // SAFETY: postado por `enviar` com um `EventCommand`.
                let c = unsafe { Box::from_raw(ov.cast::<EventCommand>()) };
                handle_command(*c);
            } else {
                let error = if ok { 0 } else { last_system_error() };
                // SAFETY: o `OVERLAPPED` é o primeiro campo de uma
                // `Operation` emitida.
                let op = unsafe { Box::from_raw(ov.cast::<Operation>()) };
                handle_completion(op, ok, bytes, error);
            }
        }
    }

    /// `HandleCompletionOrInterrupt` e `HandleIOCompletion`.
    fn handle_completion(mut op: Box<Operation>, ok: bool, bytes: u32, error: u32) {
        let Some(m) = op.handle.take() else { return };
        // O fechamento da conexão ou do handle, e as operações abortadas
        // pelo fechamento, contam como 0 bytes; o resto é erro (-1).
        let bytes: i64 = if ok {
            i64::from(bytes)
        } else if [ERROR_CONNECTION_ABORTED, ERROR_OPERATION_ABORTED, ERROR_NETNAME_DELETED, ERROR_BROKEN_PIPE].contains(&error) {
            0
        } else {
            -1
        };
        let mut e = m.estado();
        match op.tipo {
            OperationKind::Accept => finish_accept(&m, &mut e, op),
            OperationKind::Read => {
                op.size = bytes.max(0) as usize;
                op.ix = 0;
                e.pending_read = false;
                if !e.closing {
                    e.ready_flag = Some(op);
                }
                if bytes > 0 {
                    if !e.closing {
                        dispatch_events(&mut e, IN_MASK);
                    }
                } else {
                    e.read_closed = true;
                    if bytes == 0 {
                        notify_close(&mut e);
                    } else {
                        notify_error(&mut e, error);
                    }
                }
            }
            OperationKind::Receive => {
                e.pending_read = false;
                if bytes >= 0 {
                    op.size = bytes as usize;
                    op.ix = 0;
                    if !e.closing {
                        e.ready_flag = Some(op);
                        dispatch_events(&mut e, IN_MASK);
                    }
                } else {
                    notify_error(&mut e, error);
                }
            }
            OperationKind::Write | OperationKind::SendOp => {
                e.pending_write = false;
                if bytes >= 0 {
                    if !e.error && !e.closing {
                        dispatch_events(&mut e, OUT_MASK);
                    }
                } else {
                    notify_error(&mut e, error);
                }
            }
            OperationKind::Disconnect => finish_disconnect(&m, &mut e),
            OperationKind::Connection => {
                if bytes < 0 {
                    notify_error(&mut e, error);
                } else {
                    finish_connect(&m, &mut e);
                }
                e.connected = true;
            }
        }
        notify_destroy_if_closed(&m, &mut e);
    }

    /// `ListenSocket::AcceptComplete`: a conexão aceita entra na fila e o
    /// Dart é avisado.
    fn finish_accept(m: &Arc<Handle>, e: &mut State, mut op: Box<Operation>) {
        if !e.closing {
            let listener = m.raw;
            // SAFETY: herda o contexto do soquete de escuta.
            let r = unsafe {
                setsockopt(op.client, SOL_SOCKET, SO_UPDATE_ACCEPT_CONTEXT, (&listener as *const Bruto).cast(), std::mem::size_of::<Bruto>() as i32)
            };
            if r == 0 {
                let client = std::mem::replace(&mut op.client, INVALID);
                // O `getpeername` não vale para um soquete aceito com E/S
                // sobreposta: o endereço remoto vem do `AcceptEx`.
                let (mut local, mut remote) = (std::ptr::null_mut(), std::ptr::null_mut());
                let (mut n_local, mut n_remote) = (0i32, 0i32);
                // SAFETY: o buffer é o que o `AcceptEx` preencheu.
                unsafe {
                    (extensions().accept_addresses)(
                        op.data.as_mut_ptr(),
                        0,
                        ACCEPT_ADDRESS_SIZE,
                        ACCEPT_ADDRESS_SIZE,
                        &mut local,
                        &mut n_local,
                        &mut remote,
                        &mut n_remote,
                    )
                };
                let c = Handle::new(HandleKind::Client, client);
                {
                    let mut ce = c.estado();
                    ce.connected = true;
                    if !remote.is_null() {
                        ce.remote = Some(OsAddress::from_pointer(remote, n_remote.max(0) as u32));
                    }
                }
                e.accepted_list.push_back(c);
            }
        }
        e.pending_accepts -= 1;
        dispatch_accepts(e);
    }

    /// `HandleInterrupt`: um comando do Dart para um soquete.
    fn handle_command(c: EventCommand) {
        // O comando carrega uma referência do soquete (solta ao fim).
        // SAFETY: `c.sock` veio de `NativeSocket::new`, retido para o
        // comando.
        let sock = unsafe { Arc::from_raw(c.sock as *const NativeSocket) };
        let fd = sock.descriptor();
        if fd < 0 {
            return;
        }
        if is_command(c.data, COMMAND_CLOSE) && c.data & (1 << SIGNAL_SOCKET) != 0 {
            clear_signal_by_descriptor(fd);
        }
        // SAFETY: o objeto mantém a referência enquanto o descritor vale.
        let m = unsafe { retained_handle(fd) };
        let mut e = m.estado();
        if is_command(c.data, COMMAND_RETURN_TOKENS) {
            e.info.return_tokens(c.port, c.data & ((1 << COMMAND_CLOSE) - 1));
        } else if is_command(c.data, COMMAND_MASK) {
            let events = c.data & EVENTS_MASK;
            e.info.set_port_and_mask(c.port, events);
            if m.tipo == HandleKind::Listen {
                dispatch_accepts(&mut e);
            } else {
                if e.info.mask() & IN_MASK != 0 && (m.tipo != HandleKind::Client || e.connected) {
                    issue_read(&m, &mut e);
                }
                // Pronto para escrever se nada está em andamento (o cliente,
                // depois de conectado).
                if events & OUT_MASK != 0 && !e.pending_write && (m.tipo != HandleKind::Client || e.connected) {
                    dispatch_events(&mut e, OUT_MASK);
                }
                let has_data = e.ready_flag.as_ref().is_some_and(|p| p.remaining() > 0 || m.tipo == HandleKind::Datagram);
                if events & IN_MASK != 0 && has_data {
                    dispatch_events(&mut e, IN_MASK);
                }
            }
        } else if is_command(c.data, COMMAND_SHUTDOWN_READ) {
            if m.tipo == HandleKind::Client {
                // SAFETY: soquete aberto deste manipulador.
                unsafe { shutdown(m.raw, SD_RECEIVE) };
                e.read_closed = true;
            }
        } else if is_command(c.data, COMMAND_SHUTDOWN_WRITE) {
            if m.tipo == HandleKind::Client {
                // SAFETY: soquete aberto deste manipulador.
                unsafe { shutdown(m.raw, SD_SEND) };
                e.write_closed = true;
            }
        } else if is_command(c.data, COMMAND_CLOSE) {
            let mut do_close = true;
            if m.tipo == HandleKind::Listen && !listen_record().do_close(c.sock) {
                // Outros objetos escutam no mesmo soquete do sistema: este
                // sai, e o soquete fica.
                do_close = false;
                e.info.remove_port(c.port);
                post_event(c.port, 1 << EVENT_DESTROYED);
                sock.release_descriptor();
            }
            if do_close {
                // A porta recebe o aviso de destruição.
                e.info.set_port_and_mask(c.port, 0);
                close_handle(&m, &mut e);
                sock.release_descriptor();
            }
        }
        notify_destroy_if_closed(&m, &mut e);
    }
}

#[cfg(windows)]
use io_windows::*;
