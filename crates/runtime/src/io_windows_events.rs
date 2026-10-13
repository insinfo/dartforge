// Native runtime: the `dart:io` event handler on Windows (the VM's
// `runtime/bin/eventhandler_win.cc`), on top of an I/O completion port (IOCP).
//
// On Windows the "descriptor" of a `_NativeSocket` is a [`Handle`] (the VM's
// `Handle`): the system `SOCKET`/`HANDLE`, the Dart tokens and ports
// ([`DescriptorInfo`], the same as on Unix) and the overlapped I/O in progress.
// There is no readiness to watch as with epoll: the handler keeps one read issued
// (`WSARecv`/`ReadFile`/`AcceptEx`) and keeps what arrived; Dart reads from that
// buffer (`Socket_Read`), and each read that drains it issues the next one. A write
// copies the bytes and issues `WSASend`/`WriteFile`; until it completes the socket
// is "full" (writes 0). The completions and Dart's commands arrive through the
// same completion port and are served by a single thread, as in the VM.
//
// Standard input does not accept overlapped I/O: a thread does the synchronous
// `ReadFile` and posts the completion to the port.
//
// Everything Windows-specific stays in this module; the runtime root only sees the
// names that the common natives use.

#[cfg(windows)]
mod io_windows {
    use super::*;
    use std::ffi::c_void;
    use std::sync::Arc;

    /// A system `HANDLE` or `SOCKET`.
    pub(super) type RawHandle = usize;
    /// `INVALID_HANDLE_VALUE` and `INVALID_SOCKET`.
    pub(super) const INVALID: RawHandle = usize::MAX;

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

    /// The size of the read buffer (`kBufferSize`), which holds a whole UDP
    /// datagram.
    const BUFFER_SIZE: usize = 64 * 1024;
    /// What one console `ReadFile` reads at a time (`kStdOverlappedBufferSize`).
    const CONSOLE_READ_SIZE: usize = 16 * 1024;
    /// The largest UDP datagram (`kMaxUDPPackageLength`).
    const MAX_DATAGRAM_SIZE: usize = 64 * 1024;
    /// The space of each `AcceptEx` address: a `SOCKADDR_STORAGE` plus 16 bytes
    /// (`kAcceptExAddressStorageSize`).
    const ACCEPT_ADDRESS_SIZE: u32 = 128 + 16;
    /// The `AcceptEx`s that a listening socket keeps issued (`kMinIssuedAccepts`).
    const ACCEPTS_ISSUED: usize = 5;

    /// The completion key of Dart's commands (the `InterruptMessage`); that of I/O
    /// is [`IO_KEY`].
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
        /// An `OVERLAPPED` with the given event.
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
        fn CreateIoCompletionPort(file: RawHandle, existing: usize, key: usize, threads: u32) -> usize;
        fn GetQueuedCompletionStatus(port: usize, bytes: *mut u32, key: *mut usize, ov: *mut *mut OverlappedOp, wait_entry: u32) -> i32;
        fn PostQueuedCompletionStatus(port: usize, bytes: u32, key: usize, ov: *mut OverlappedOp) -> i32;
        pub(super) fn CloseHandle(h: RawHandle) -> i32;
        pub(super) fn ReadFile(h: RawHandle, b: *mut u8, n: u32, read_list: *mut u32, ov: *mut OverlappedOp) -> i32;
        pub(super) fn WriteFile(h: RawHandle, b: *const u8, n: u32, written: *mut u32, ov: *mut OverlappedOp) -> i32;
        fn CancelIoEx(h: RawHandle, ov: *mut OverlappedOp) -> i32;
        pub(super) fn GetLastError() -> u32;
        pub(super) fn GetFileType(h: RawHandle) -> u32;
        fn GetStdHandle(which: u32) -> RawHandle;
        fn ReadDirectoryChangesW(
            directory: RawHandle,
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
        fn WSASocketW(family: i32, ty: i32, protocol: i32, info: *const c_void, group: u32, flag_bits: u32) -> RawHandle;
        pub(super) fn closesocket(s: RawHandle) -> i32;
        fn shutdown(s: RawHandle, how: i32) -> i32;
        pub(super) fn bind(s: RawHandle, e: *const u8, n: i32) -> i32;
        pub(super) fn listen(s: RawHandle, queue: i32) -> i32;
        pub(super) fn setsockopt(s: RawHandle, level: i32, option: i32, v: *const u8, n: i32) -> i32;
        pub(super) fn getsockopt(s: RawHandle, level: i32, option: i32, v: *mut u8, n: *mut i32) -> i32;
        pub(super) fn getsockname(s: RawHandle, e: *mut u8, n: *mut i32) -> i32;
        pub(super) fn getpeername(s: RawHandle, e: *mut u8, n: *mut i32) -> i32;
        fn WSARecv(s: RawHandle, b: *mut BufferWsa, n: u32, received_list: *mut u32, flag_bits: *mut u32, ov: *mut OverlappedOp, routine: usize) -> i32;
        fn WSASend(s: RawHandle, b: *mut BufferWsa, n: u32, sent: *mut u32, flag_bits: u32, ov: *mut OverlappedOp, routine: usize) -> i32;
        fn WSARecvFrom(
            s: RawHandle,
            b: *mut BufferWsa,
            n: u32,
            received_list: *mut u32,
            flag_bits: *mut u32,
            of: *mut u8,
            from_n: *mut i32,
            ov: *mut OverlappedOp,
            routine: usize,
        ) -> i32;
        fn WSASendTo(s: RawHandle, b: *mut BufferWsa, n: u32, sent: *mut u32, flag_bits: u32, to: *const u8, to_len: i32, ov: *mut OverlappedOp, routine: usize) -> i32;
        fn WSAIoctl(s: RawHandle, code: u32, ent: *const c_void, ent_n: u32, out: *mut c_void, out_len: u32, bytes: *mut u32, ov: *mut OverlappedOp, routine: usize) -> i32;
    }

    // -----------------------------------------------------------------------
    // Winsock and the extensions (`AcceptEx`, `ConnectEx`…).

    /// `SocketBase::Initialize`: `WSAStartup`, once per process.
    pub(super) fn start_winsock() {
        static STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        STARTED.get_or_init(|| {
            let mut data = [0u8; 512];
            // SAFETY: `WSADATA` fits in 512 bytes.
            if unsafe { WSAStartup(0x0202, data.as_mut_ptr()) } != 0 {
                panic!("dart:io: failed to start Winsock ({})", unsafe { WSAGetLastError() });
            }
        });
    }

    type FnAcceptEx = unsafe extern "system" fn(RawHandle, RawHandle, *mut u8, u32, u32, u32, *mut u32, *mut OverlappedOp) -> i32;
    type FnConnectEx = unsafe extern "system" fn(RawHandle, *const u8, i32, *const u8, u32, *mut u32, *mut OverlappedOp) -> i32;
    type FnDisconnectEx = unsafe extern "system" fn(RawHandle, *mut OverlappedOp, u32, u32) -> i32;
    type FnGetAcceptExSockaddrs = unsafe extern "system" fn(*mut u8, u32, u32, u32, *mut *mut u8, *mut i32, *mut *mut u8, *mut i32);

    /// The Winsock extension functions (`InitializeSocketExtensions`).
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
            // SAFETY: a provisional socket only to query the extensions.
            let s = unsafe { WSASocketW(i32::from(AF_INET), SOCK_STREAM, IPPROTO_TCP, std::ptr::null(), 0, WSA_FLAG_OVERLAPPED) };
            if s == INVALID {
                panic!("dart:io: failed to create the Winsock extensions socket");
            }
            let function = |guid: Guid| -> usize {
                let mut f = 0usize;
                let mut n = 0u32;
                // SAFETY: queries the pointer of extension `guid`.
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
                    panic!("dart:io: Winsock extension unavailable");
                }
                f
            };
            // SAFETY: the pointers came from `WSAIoctl` with each function's GUIDs
            // (`WSAID_*`), with these signatures.
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
            // SAFETY: the provisional socket is no longer used.
            unsafe { closesocket(s) };
            e
        })
    }

    /// An overlapped socket that children do not inherit.
    pub(super) fn new_socket(family: u16, ty: i32, protocol: i32) -> IoResult<RawHandle> {
        start_winsock();
        // SAFETY: creates a socket.
        let s = unsafe {
            WSASocketW(i32::from(family), ty, protocol, std::ptr::null(), 0, WSA_FLAG_OVERLAPPED | WSA_FLAG_NO_HANDLE_INHERIT)
        };
        if s == INVALID {
            return Err(OsError::from_code(unsafe { WSAGetLastError() }));
        }
        Ok(s)
    }

    // -----------------------------------------------------------------------
    // The overlapped operations (`OverlappedBuffer`).

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

    /// An operation in progress: the `OVERLAPPED` (the first field, so that the
    /// completion finds the operation), the buffer and the reference of the
    /// [`Handle`] that it keeps alive until it completes.
    #[repr(C)]
    struct Operation {
        overlapped: OverlappedOp,
        ty: OperationKind,
        handle: Option<Arc<Handle>>,
        data: Vec<u8>,
        /// The next byte to read and how many there are (reads).
        ix: usize,
        size: usize,
        buffer_wsa: BufferWsa,
        flag_bits: u32,
        /// The source address (`WSARecvFrom`) or the destination (`WSASendTo`).
        address: [u8; 128],
        address_size: i32,
        /// The socket that `AcceptEx` fills.
        client: RawHandle,
    }

    impl Operation {
        fn new_area(ty: OperationKind, m: &Arc<Handle>, capacity: usize) -> Box<Operation> {
            Box::new(Operation {
                overlapped: OverlappedOp::default(),
                ty,
                handle: Some(Arc::clone(m)),
                data: vec![0; capacity],
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

        /// Hands the operation to the system, which returns it on completion.
        fn issued(self: Box<Self>) {
            let _ = Box::into_raw(self);
        }
    }

    // SAFETY: the operation's only pointer (`buffer_wsa.data`) points to its own
    // `data`; the operation passes between threads whole (Dart's, the system's and
    // the handler's), never shared.
    unsafe impl Send for Operation {}

    impl Drop for Operation {
        fn drop(&mut self) {
            if self.client != INVALID {
                // SAFETY: the `AcceptEx` socket that nobody adopted.
                unsafe { closesocket(self.client) };
            }
        }
    }

    // -----------------------------------------------------------------------
    // The `Handle`.

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum HandleKind {
        /// An overlapped file or pipe (the pipes of processes and signals).
        File,
        /// Standard input, without overlapped I/O.
        Pattern,
        Client,
        Listen,
        Datagram,
        /// A watched directory (`DirectoryWatchHandle`): the read is
        /// `ReadDirectoryChangesW`.
        Directory,
    }

    /// The VM's `Handle`.
    pub(super) struct Handle {
        pub(super) ty: HandleKind,
        pub(super) raw: RawHandle,
        overlapped: bool,
        /// Watched directory: the `ReadDirectoryChangesW` filter
        /// (`FILE_NOTIFY_CHANGE_*`) and whether the subtree is included.
        watch: (u32, bool),
        state: std::sync::Mutex<State>,
        /// Wakes the synchronous read thread (standard input).
        read_request: std::sync::Condvar,
    }

    pub(super) struct State {
        /// `kClosing`, `kCloseRead`, `kCloseWrite`, `kError`.
        closing: bool,
        read_closed: bool,
        write_closed: bool,
        error: bool,
        /// The system object still needs to be closed.
        is_open: bool,
        info: DescriptorInfo,
        /// The data read that Dart has not consumed yet (`data_ready_`).
        ready_flag: Option<Box<Operation>>,
        pending_read: bool,
        pending_write: bool,
        pub(super) last_error: u32,
        // Client.
        connected: bool,
        disconnected: bool,
        pub(super) remote: Option<OsAddress>,
        // Listen.
        pub(super) family: u16,
        pending_accepts: usize,
        accepted_list: std::collections::VecDeque<Arc<Handle>>,
        // Standard input: the read handed to the thread and whether it exists.
        sync_read: Option<Box<Operation>>,
        reader_thread: bool,
    }

    impl State {
        /// The handle passes to the caller (`Process::Wait`): the handler neither
        /// closes it nor does more I/O on it.
        pub(super) fn take_handle(&mut self) {
            self.closing = true;
            self.is_open = false;
        }
    }

    impl Handle {
        /// Creates the handle object of `raw`, associated with the completion port
        /// when it does overlapped I/O.
        pub(super) fn new(ty: HandleKind, raw: RawHandle) -> Arc<Handle> {
            Handle::new_with(ty, raw, (0, false))
        }

        fn new_with(ty: HandleKind, raw: RawHandle, watch: (u32, bool)) -> Arc<Handle> {
            let overlapped = ty != HandleKind::Pattern;
            let m = Arc::new(Handle {
                ty,
                raw,
                overlapped,
                watch,
                state: std::sync::Mutex::new(State {
                    closing: false,
                    read_closed: false,
                    write_closed: false,
                    error: false,
                    is_open: true,
                    info: DescriptorInfo::new(ty == HandleKind::Listen),
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
                // SAFETY: associates an open handle with the completion port.
                if unsafe { CreateIoCompletionPort(raw, port, IO_KEY, 0) } == 0 {
                    panic!("dart:io: failed to associate the handle with the completion port ({})", unsafe { GetLastError() });
                }
            }
            m
        }

        pub(super) fn state(&self) -> std::sync::MutexGuard<'_, State> {
            self.state.lock().unwrap_or_else(|e| e.into_inner())
        }

        fn is_socket(&self) -> bool {
            matches!(self.ty, HandleKind::Client | HandleKind::Listen | HandleKind::Datagram)
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            let e = self.state.get_mut().unwrap_or_else(|e| e.into_inner());
            if e.is_open {
                // SAFETY: the system object belongs to this handle object, which
                // has not closed it.
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

    /// The Dart descriptor of a handle object (the reference passes to the object).
    pub(super) fn descriptor_of(m: Arc<Handle>) -> i64 {
        Arc::into_raw(m) as i64
    }

    /// The handle object of a descriptor, with a new reference.
    ///
    /// # Safety
    /// `d` came from [`descriptor_of`] and its reference still exists.
    pub(super) unsafe fn retained_handle(d: i64) -> Arc<Handle> {
        let p = d as *const Handle;
        // SAFETY: guaranteed by the caller.
        unsafe {
            Arc::increment_strong_count(p);
            Arc::from_raw(p)
        }
    }

    /// Windows's `Socket::CloseFd`: releases the object's reference.
    pub(super) fn close_socket_descriptor(d: i64) {
        // SAFETY: the object's reference, released once (the descriptor becomes -1
        // before).
        drop(unsafe { Arc::from_raw(d as *const Handle) });
    }

    pub(super) fn retain_descriptor(d: i64) -> i64 {
        // SAFETY: `d` is the live descriptor of a registered listening socket.
        std::mem::forget(unsafe { retained_handle(d) });
        d
    }

    /// The handle object of a descriptor that the Dart object keeps alive during the
    /// native (`None` if the object has already released it: -1).
    pub(super) fn handle_of(d: i64) -> Option<Arc<Handle>> {
        // SAFETY: a non-negative descriptor is a live reference of the object.
        (d >= 0).then(|| unsafe { retained_handle(d) })
    }

    /// The handle object of a Dart object's socket (`None` if closed).
    pub(super) fn socket_handle(s: &NativeSocket) -> Option<Arc<Handle>> {
        handle_of(s.descriptor())
    }

    /// `ERROR_INVALID_HANDLE`: the socket was already closed.
    pub(super) fn invalid_handle() -> OsError {
        OsError::from_code(6)
    }

    // -----------------------------------------------------------------------
    // Events for Dart.

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

    /// `HandleIssueError`: the end of the pipe or a dropped connection is a close;
    /// the rest is an error.
    fn issue_failure(m: &Handle, e: &mut State, code: u32) {
        let end = if m.is_socket() { WSAECONNRESET } else { ERROR_BROKEN_PIPE };
        if code == end {
            notify_close(e);
        } else {
            notify_error(e, code);
        }
    }

    /// `IsClosed` of each type.
    fn is_closed(m: &Handle, e: &State) -> bool {
        match m.ty {
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
    // Issuing I/O (with the state locked).

    fn last_system_error() -> u32 {
        // SAFETY: reads the thread's error.
        unsafe { GetLastError() }
    }

    /// `IssueReadLocked`: keeps a read in progress while Dart has no data to
    /// consume.
    fn issue_read(m: &Arc<Handle>, e: &mut State) -> bool {
        match m.ty {
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
            // The read thread does the synchronous `ReadFile` and posts the
            // completion.
            e.sync_read = Some(op);
            if !e.reader_thread {
                e.reader_thread = true;
                let m2 = Arc::clone(m);
                std::thread::Builder::new()
                    .name("dart:io ReadFile".to_string())
                    .spawn(move || sync_read_loop(&m2))
                    .expect("dart:io: failed to create the read thread");
            }
            m.read_request.notify_one();
            return true;
        }
        let ok = if m.ty == HandleKind::Client {
            let b = op.buffer_wsa();
            let f: *mut u32 = &mut op.flag_bits;
            let ov = op.clean_overlapped();
            // SAFETY: the operation (buffer, flags, `OVERLAPPED`) lives until the
            // completion.
            unsafe { WSARecv(m.raw, b, 1, std::ptr::null_mut(), f, ov, 0) == 0 }
        } else if m.ty == HandleKind::Directory {
            // `DirectoryWatchHandle::IssueReadLocked`: the `FILE_NOTIFY_INFORMATION`
            // records arrive in the operation's buffer.
            let (filter_obj, subtree) = m.watch;
            let (p, n) = (op.data.as_mut_ptr(), op.data.len() as u32);
            let ov = op.clean_overlapped();
            // SAFETY: as above. `ReadDirectoryChangesW` requires the buffer aligned
            // to `DWORD`; the `Vec`'s comes from `HeapAlloc` (the system allocator on
            // Windows), aligned to 16.
            unsafe { ReadDirectoryChangesW(m.raw, p, n, i32::from(subtree), filter_obj, std::ptr::null_mut(), ov, 0) != 0 }
        } else {
            let (p, n) = (op.data.as_mut_ptr(), op.data.len() as u32);
            let ov = op.clean_overlapped();
            // SAFETY: as above.
            unsafe { ReadFile(m.raw, p, n, std::ptr::null_mut(), ov) != 0 }
        };
        let code = last_system_error();
        if ok || code == ERROR_IO_PENDING {
            op.issued();
            return true;
        }
        e.pending_read = false;
        drop(op);
        // The VM's `DirectoryWatchHandle::IssueReadLocked` returns the failure
        // without notifying Dart (a file in place of the directory, for example,
        // gets no events); the other handle objects notify.
        if m.ty != HandleKind::Directory {
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
        let of = op.address.as_mut_ptr();
        let from_n: *mut i32 = &mut op.address_size;
        let ov = op.clean_overlapped();
        e.pending_read = true;
        // SAFETY: the operation lives until the completion.
        let ok = unsafe { WSARecvFrom(m.raw, b, 1, std::ptr::null_mut(), f, of, from_n, ov, 0) == 0 };
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
        // SAFETY: the operation (the addresses buffer, `OVERLAPPED`) lives until the
        // completion.
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

    /// The `ConnectEx` of `Socket::CreateConnect`: completes through the port, even
    /// when it finishes immediately.
    pub(super) fn start_connect(m: &Arc<Handle>, dest: &OsAddress) -> IoResult<()> {
        let mut op = Operation::new_area(OperationKind::Connection, m, 0);
        let ov = op.clean_overlapped();
        // SAFETY: `dest` is a valid `sockaddr`; the operation lives until the
        // completion.
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

    /// `ListenSocket::StartAccept`: `false` with the error in `last_error`.
    pub(super) fn start_accepting(m: &Arc<Handle>) -> bool {
        let mut e = m.state();
        (0..ACCEPTS_ISSUED).all(|_| issue_accept(m, &mut e))
    }

    /// `ClientSocket::IssueDisconnectLocked`: the disconnection and the destruction
    /// notice to the ports.
    fn issue_disconnect(m: &Arc<Handle>, e: &mut State) {
        let mut op = Operation::new_area(OperationKind::Disconnect, m, 0);
        let ov = op.clean_overlapped();
        // SAFETY: the operation lives until the completion.
        let ok = unsafe { (extensions().disconnect)(m.raw, ov, TF_REUSE_SOCKET, 0) != 0 };
        let code = last_system_error();
        if ok || code == ERROR_IO_PENDING {
            // Even when it completes immediately, the completion goes through the port.
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
            // SAFETY: the socket belongs to this handle object.
            unsafe { closesocket(m.raw) };
            e.is_open = false;
        }
        e.ready_flag = None;
        e.disconnected = true;
    }

    /// `ClientSocket::ConnectComplete`.
    fn finish_connect(m: &Arc<Handle>, e: &mut State) {
        // SAFETY: updates the context of the socket connected by `ConnectEx`.
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
            // Unblocks the synchronous `ReadFile` of the read thread.
            // SAFETY: cancels the handle's I/O.
            unsafe { CancelIoEx(m.raw, std::ptr::null_mut()) };
        }
        if e.closing {
            return;
        }
        e.closing = true;
        match m.ty {
            HandleKind::File | HandleKind::Pattern | HandleKind::Directory => {
                if e.is_open {
                    // SAFETY: the handle belongs to this handle object; the pending
                    // I/O completes with `ERROR_OPERATION_ABORTED` (that of the
                    // watched directory too: `DirectoryWatchHandle::Stop`).
                    unsafe { CloseHandle(m.raw) };
                    e.is_open = false;
                }
                if m.ty == HandleKind::Pattern {
                    m.read_request.notify_all();
                    let mut g = stdin_entry();
                    if g.as_ref().is_some_and(|x| std::ptr::eq(Arc::as_ptr(x), Arc::as_ptr(m))) {
                        *g = None;
                    }
                }
            }
            HandleKind::Listen => {
                if e.is_open {
                    // SAFETY: the pending `AcceptEx`s complete aborted.
                    unsafe { closesocket(m.raw) };
                    e.is_open = false;
                }
                for c in std::mem::take(&mut e.accepted_list) {
                    let mut ce = c.state();
                    close_handle(&c, &mut ce);
                    notify_destroy_if_closed(&c, &mut ce);
                }
            }
            HandleKind::Client => {
                // SAFETY: shuts down both directions before disconnecting.
                unsafe { shutdown(m.raw, SD_BOTH) };
                issue_disconnect(m, e);
            }
            HandleKind::Datagram => {
                if e.is_open {
                    // SAFETY: the pending I/O completes aborted.
                    unsafe { closesocket(m.raw) };
                    e.is_open = false;
                }
                e.read_closed = true;
                e.write_closed = true;
            }
        }
    }

    /// Closes a handle object that no Dart object got to use.
    pub(super) fn discard(m: &Arc<Handle>) {
        let mut e = m.state();
        match m.ty {
            HandleKind::Client => {
                // Never connected: closes right away, without `DisconnectEx`.
                e.closing = true;
                finish_disconnect(m, &mut e);
            }
            _ => close_handle(m, &mut e),
        }
    }

    /// The synchronous read thread (`Handle::ReadSyncCompleteAsync`): serves one
    /// request at a time and posts the completion to the port.
    fn sync_read_loop(m: &Arc<Handle>) {
        let port = event_handler().port;
        loop {
            let mut op = {
                let mut e = m.state();
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
            // SAFETY: queries the handle's type.
            let n = if unsafe { GetFileType(m.raw) } == FILE_TYPE_CHAR { CONSOLE_READ_SIZE } else { op.data.len() };
            let mut read_list = 0u32;
            // SAFETY: a synchronous read into the operation's buffer.
            if unsafe { ReadFile(m.raw, op.data.as_mut_ptr(), n as u32, &mut read_list, std::ptr::null_mut()) } == 0 {
                read_list = 0;
            }
            let ov = op.clean_overlapped();
            op.issued();
            // SAFETY: the operation comes back through the port, like an overlapped one.
            if unsafe { PostQueuedCompletionStatus(port, read_list, IO_KEY, ov) } == 0 {
                panic!("dart:io: PostQueuedCompletionStatus failed ({})", unsafe { GetLastError() });
            }
        }
    }

    // -----------------------------------------------------------------------
    // Standard input.

    fn stdin_entry() -> std::sync::MutexGuard<'static, Option<Arc<Handle>>> {
        static E: std::sync::Mutex<Option<Arc<Handle>>> = std::sync::Mutex::new(None);
        E.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `SocketBase::GetStdioHandle`: only the input (`StdHandle::Stdin`, one per
    /// process).
    pub(super) fn default_handle(num: i64) -> i64 {
        if num != 0 {
            return -1;
        }
        // SAFETY: reads the process's handle (STD_INPUT_HANDLE = -10).
        let h = unsafe { GetStdHandle(-10i32 as u32) };
        if h == INVALID || h == 0 {
            return -1;
        }
        let mut g = stdin_entry();
        let m = Arc::clone(g.get_or_insert_with(|| Handle::new(HandleKind::Pattern, h)));
        descriptor_of(m)
    }

    // -----------------------------------------------------------------------
    // The operations the natives do (`Handle::Read`, `Write`…).

    /// `Handle::Available`.
    pub(super) fn available(s: &NativeSocket) -> i64 {
        socket_handle(s).map_or(0, |m| m.state().ready_flag.as_ref().map_or(0, |p| p.remaining() as i64))
    }

    /// `Handle::Read`: what has already arrived; draining the buffer issues the next
    /// read.
    pub(super) fn read_from(s: &NativeSocket, dest: &mut [u8]) -> IoResult<usize> {
        let Some(m) = socket_handle(s) else { return Ok(0) };
        read_from_handle(&m, dest)
    }

    fn read_from_handle(m: &Arc<Handle>, dest: &mut [u8]) -> IoResult<usize> {
        let mut e = m.state();
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
    // Directory watching (`file_system_watcher_win.cc`).

    /// `FILE_LIST_DIRECTORY`.
    const FILE_LIST_DIRECTORY: u32 = 0x0001;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;

    /// `FileSystemWatcher::WatchPath`: opens the directory for overlapped I/O,
    /// creates the handle object (the `DirectoryWatchHandle`, associated with the
    /// completion port) and issues the first read right away, so that events count
    /// from now on (`Start`). The returned descriptor is the path id and that of the
    /// internal socket Dart creates with it.
    pub(super) fn watch_directory(file_path: &str, filter_obj: u32, recursive: bool) -> IoResult<i64> {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::IntoRawHandle;
        let file = std::fs::OpenOptions::new()
            .access_mode(FILE_LIST_DIRECTORY)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED)
            .open(file_path)
            .map_err(|e| OsError::of(&e))?;
        let m = Handle::new_with(HandleKind::Directory, file.into_raw_handle() as RawHandle, (filter_obj, recursive));
        {
            let mut e = m.state();
            issue_read(&m, &mut e);
        }
        Ok(descriptor_of(m))
    }

    /// `FileSystemWatcher::UnwatchPath` (`DirectoryWatchHandle::Stop`): cancels the
    /// read and closes the handle; the reference stays with the internal socket,
    /// which Dart closes right after.
    pub(super) fn stop_watch(d: i64) {
        if let Some(m) = handle_of(d) {
            let mut e = m.state();
            close_handle(&m, &mut e);
            notify_destroy_if_closed(&m, &mut e);
        }
    }

    /// `FileSystemWatcher::ReadEvents`: the records that have already arrived
    /// (`Available` and `Read`; draining issues the next read).
    pub(super) fn directory_events(d: i64) -> IoResult<Vec<u8>> {
        let Some(m) = handle_of(d) else { return Ok(Vec::new()) };
        let available = m.state().ready_flag.as_ref().map_or(0, |p| p.remaining());
        let mut buf = vec![0u8; available];
        let n = read_from_handle(&m, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    /// `Handle::Write`: copies up to 64 KiB and issues the write; 0 while another
    /// one is in progress.
    pub(super) fn write_to_handle(m: &Arc<Handle>, data: &[u8]) -> IoResult<usize> {
        let mut e = m.state();
        if e.pending_write || is_closed(m, &e) {
            return Ok(0);
        }
        let n = data.len().min(BUFFER_SIZE);
        if !m.overlapped {
            // Standard input is not written by Dart; a synchronous handle writes
            // right away.
            let mut written = 0u32;
            // SAFETY: a synchronous write of the given bytes.
            if unsafe { WriteFile(m.raw, data.as_ptr(), n as u32, &mut written, std::ptr::null_mut()) } == 0 {
                return Err(OsError::from_code(last_system_error() as i32));
            }
            return Ok(written as usize);
        }
        let mut op = Operation::new_area(OperationKind::Write, m, 0);
        op.data = data[..n].to_vec();
        op.size = n;
        e.pending_write = true;
        let ok = if m.ty == HandleKind::Client {
            let b = op.buffer_wsa();
            let ov = op.clean_overlapped();
            // SAFETY: the operation lives until the completion.
            unsafe { WSASend(m.raw, b, 1, std::ptr::null_mut(), 0, ov, 0) == 0 }
        } else {
            let p = op.data.as_ptr();
            let ov = op.clean_overlapped();
            // SAFETY: as above.
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
        socket_handle(s).is_some_and(|m| m.state().pending_write)
    }

    /// `ServerSocket::Accept`: the next accepted connection (its descriptor) or -1;
    /// replenishes the issued `AcceptEx`s.
    pub(super) fn accept_conn(fd: i64) -> i64 {
        let Some(m) = handle_of(fd) else { return -1 };
        let mut e = m.state();
        let r = e.accepted_list.pop_front();
        if !e.closing && e.pending_accepts < ACCEPTS_ISSUED {
            issue_accept(&m, &mut e);
        }
        r.map_or(-1, descriptor_of)
    }

    /// `SocketBase::AvailableDatagram`.
    pub(super) fn has_datagram(fd: i64) -> bool {
        handle_of(fd).is_some_and(|m| m.state().ready_flag.is_some())
    }

    /// `Handle::RecvFrom`: the received datagram (the whole buffer comes out, like
    /// `recvfrom`) and the source.
    pub(super) fn receive_datagram(fd: i64, maximum: usize) -> IoResult<Option<(Vec<u8>, OsAddress)>> {
        let Some(m) = handle_of(fd) else { return Ok(None) };
        let mut e = m.state();
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
        let mut e = m.state();
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
        // SAFETY: the operation (data, address, `OVERLAPPED`) lives until the
        // completion.
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
    // The handler thread.

    pub(super) fn start_handler() -> EventHandler {
        // SAFETY: creates the completion port (one concurrent thread).
        let port = unsafe { CreateIoCompletionPort(INVALID, 0, 0, 1) };
        if port == 0 {
            panic!("dart:io: failed to create the completion port ({})", unsafe { GetLastError() });
        }
        std::thread::Builder::new()
            .name("dart:io EventHandler".to_string())
            .spawn(move || event_loop(port))
            .expect("dart:io: failed to create the event handler thread");
        EventHandler { port }
    }

    impl EventHandler {
        /// `SendData`: the command goes through the completion port.
        pub(super) fn send(&self, c: EventCommand) {
            let p = Box::into_raw(Box::new(c));
            // SAFETY: the loop receives the pointer with the command key and takes
            // it back.
            if unsafe { PostQueuedCompletionStatus(self.port, 0, COMMAND_KEY, p.cast()) } == 0 {
                panic!("dart:io: PostQueuedCompletionStatus failed ({})", unsafe { GetLastError() });
            }
        }
    }

    fn event_loop(port: usize) {
        loop {
            let mut bytes = 0u32;
            let mut key = 0usize;
            let mut ov: *mut OverlappedOp = std::ptr::null_mut();
            // SAFETY: waits for a completion on the port.
            let ok = unsafe { GetQueuedCompletionStatus(port, &mut bytes, &mut key, &mut ov, INFINITE) } != 0;
            if ov.is_null() {
                continue;
            }
            if key == COMMAND_KEY {
                // SAFETY: posted by `send` with an `EventCommand`.
                let c = unsafe { Box::from_raw(ov.cast::<EventCommand>()) };
                handle_command(*c);
            } else {
                let error = if ok { 0 } else { last_system_error() };
                // SAFETY: the `OVERLAPPED` is the first field of an issued
                // `Operation`.
                let op = unsafe { Box::from_raw(ov.cast::<Operation>()) };
                handle_completion(op, ok, bytes, error);
            }
        }
    }

    /// `HandleCompletionOrInterrupt` and `HandleIOCompletion`.
    fn handle_completion(mut op: Box<Operation>, ok: bool, bytes: u32, error: u32) {
        let Some(m) = op.handle.take() else { return };
        // The closing of the connection or the handle, and the operations aborted
        // by the closing, count as 0 bytes; the rest is an error (-1).
        let bytes: i64 = if ok {
            i64::from(bytes)
        } else if [ERROR_CONNECTION_ABORTED, ERROR_OPERATION_ABORTED, ERROR_NETNAME_DELETED, ERROR_BROKEN_PIPE].contains(&error) {
            0
        } else {
            -1
        };
        let mut e = m.state();
        match op.ty {
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

    /// `ListenSocket::AcceptComplete`: the accepted connection goes into the queue
    /// and Dart is notified.
    fn finish_accept(m: &Arc<Handle>, e: &mut State, mut op: Box<Operation>) {
        if !e.closing {
            let listener = m.raw;
            // SAFETY: inherits the listening socket's context.
            let r = unsafe {
                setsockopt(op.client, SOL_SOCKET, SO_UPDATE_ACCEPT_CONTEXT, (&listener as *const RawHandle).cast(), std::mem::size_of::<RawHandle>() as i32)
            };
            if r == 0 {
                let client = std::mem::replace(&mut op.client, INVALID);
                // `getpeername` does not work for a socket accepted with overlapped
                // I/O: the remote address comes from `AcceptEx`.
                let (mut local, mut remote) = (std::ptr::null_mut(), std::ptr::null_mut());
                let (mut n_local, mut n_remote) = (0i32, 0i32);
                // SAFETY: the buffer is the one `AcceptEx` filled.
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
                    let mut ce = c.state();
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

    /// `HandleInterrupt`: a command from Dart for a socket.
    fn handle_command(c: EventCommand) {
        // The command carries a reference to the socket (released at the end).
        // SAFETY: `c.sock` came from `NativeSocket::new`, retained for the
        // command.
        let sock = unsafe { Arc::from_raw(c.sock as *const NativeSocket) };
        let fd = sock.descriptor();
        if fd < 0 {
            return;
        }
        if is_command(c.data, COMMAND_CLOSE) && c.data & (1 << SIGNAL_SOCKET) != 0 {
            clear_signal_by_descriptor(fd);
        }
        // SAFETY: the object keeps the reference while the descriptor is valid.
        let m = unsafe { retained_handle(fd) };
        let mut e = m.state();
        if is_command(c.data, COMMAND_RETURN_TOKENS) {
            e.info.return_tokens(c.port, c.data & ((1 << COMMAND_CLOSE) - 1));
        } else if is_command(c.data, COMMAND_MASK) {
            let events = c.data & EVENTS_MASK;
            e.info.set_port_and_mask(c.port, events);
            if m.ty == HandleKind::Listen {
                dispatch_accepts(&mut e);
            } else {
                if e.info.mask() & IN_MASK != 0 && (m.ty != HandleKind::Client || e.connected) {
                    issue_read(&m, &mut e);
                }
                // Ready to write if nothing is in progress (the client, after it
                // has connected).
                if events & OUT_MASK != 0 && !e.pending_write && (m.ty != HandleKind::Client || e.connected) {
                    dispatch_events(&mut e, OUT_MASK);
                }
                let has_data = e.ready_flag.as_ref().is_some_and(|p| p.remaining() > 0 || m.ty == HandleKind::Datagram);
                if events & IN_MASK != 0 && has_data {
                    dispatch_events(&mut e, IN_MASK);
                }
            }
        } else if is_command(c.data, COMMAND_SHUTDOWN_READ) {
            if m.ty == HandleKind::Client {
                // SAFETY: an open socket of this handle object.
                unsafe { shutdown(m.raw, SD_RECEIVE) };
                e.read_closed = true;
            }
        } else if is_command(c.data, COMMAND_SHUTDOWN_WRITE) {
            if m.ty == HandleKind::Client {
                // SAFETY: an open socket of this handle object.
                unsafe { shutdown(m.raw, SD_SEND) };
                e.write_closed = true;
            }
        } else if is_command(c.data, COMMAND_CLOSE) {
            let mut do_close = true;
            if m.ty == HandleKind::Listen && !listen_record().do_close(c.sock) {
                // Other objects listen on the same system socket: this one leaves,
                // and the socket stays.
                do_close = false;
                e.info.remove_port(c.port);
                post_event(c.port, 1 << EVENT_DESTROYED);
                sock.release_descriptor();
            }
            if do_close {
                // The port receives the destruction notice.
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
