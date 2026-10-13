// Native runtime: the `dart:io` processes on Windows (the VM's `process_win.cc`).
//
// The child is born from `CreateProcessW` with the command line that Dart already
// escaped (`_windowsArgumentEscape`), the environment in UTF-16 and, outside
// `inheritStdio` mode, only the three inherited I/O handles
// (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`: no other handle of the process leaks into
// the child). The child's standard I/O are named pipes: the parent's side is
// overlapped and becomes a [`Handle`] on the completion port; the child's is
// synchronous, as a program expects. The exit code arrives through the system
// thread pool (`RegisterWaitForSingleObject`), which writes it (`[code,
// negative]`, two `int32`s) into the process's exit pipe.
//
// Signals are the console events (`SetConsoleCtrlHandler`): SIGINT is Ctrl+C and
// SIGHUP is the window being closed.

#[cfg(windows)]
mod windows_processes {
    use super::*;
    use std::ffi::c_void;
    use std::sync::Arc;

    const PIPE_ACCESS_INBOUND: u32 = 0x1;
    const PIPE_ACCESS_OUTBOUND: u32 = 0x2;
    const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
    const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
    const PIPE_REJECT_REMOTE_CLIENTS: u32 = 0x8;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const FILE_WRITE_ATTRIBUTES: u32 = 0x100;
    const OPEN_EXISTING: u32 = 3;
    /// The buffer of each direction of the pipes (`libuv`'s).
    const PIPE_SIZE: u32 = 64 * 1024;
    const STARTF_USESTDHANDLES: u32 = 0x100;
    const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
    const CREATE_UNICODE_ENVIRONMENT: u32 = 0x400;
    const DETACHED_PROCESS: u32 = 0x8;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
    const WT_EXECUTEONLYONCE: u32 = 0x8;
    const INFINITE: u32 = u32::MAX;
    const PROCESS_TERMINATE: u32 = 0x1;
    const WAIT_OBJECT_0: u32 = 0;
    const ERROR_NOT_SUPPORTED: i32 = 50;
    const CTRL_C_EVENT: u32 = 0;
    const CTRL_CLOSE_EVENT: u32 = 2;

    #[repr(C)]
    struct AtributosDeSeguranca {
        size: u32,
        descriptor: *mut c_void,
        inherit: i32,
    }

    /// `STARTUPINFOW`.
    #[repr(C)]
    struct InfoDeInicio {
        cb: u32,
        reserved: *mut u16,
        desktop: *mut u16,
        title: *mut u16,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        columns: u32,
        line_list: u32,
        attribute: u32,
        flag_bits: u32,
        show: u16,
        reserved2_size: u16,
        reserved2: *mut u8,
        input: Bruto,
        output: Bruto,
        error: Bruto,
    }

    /// `STARTUPINFOEXW`.
    #[repr(C)]
    struct StartupInfoEx {
        base: InfoDeInicio,
        attributes: *mut c_void,
    }

    /// `PROCESS_INFORMATION`.
    #[repr(C)]
    struct ProcessInfo {
        process: Bruto,
        thread: Bruto,
        pid: u32,
        tid: u32,
    }

    type WaitReturn = unsafe extern "system" fn(*mut c_void, u8);
    type ConsoleHandler = unsafe extern "system" fn(u32) -> i32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateNamedPipeW(name: *const u16, opening: u32, mode: u32, instances: u32, output: u32, input: u32, wait_entry: u32, seg: *const AtributosDeSeguranca) -> Bruto;
        fn CreateFileW(name: *const u16, access: u32, share: u32, seg: *const AtributosDeSeguranca, disposition: u32, flag_bits: u32, model: Bruto) -> Bruto;
        fn InitializeProcThreadAttributeList(list: *mut c_void, n: u32, flag_bits: u32, size: *mut usize) -> i32;
        fn UpdateProcThreadAttribute(list: *mut c_void, flag_bits: u32, attribute: usize, value: *const c_void, size: usize, previous: *mut c_void, ret: *mut usize) -> i32;
        fn DeleteProcThreadAttributeList(list: *mut c_void);
        fn CreateProcessW(
            application: *const u16,
            line: *mut u16,
            process_attributes: *const c_void,
            thread_attributes: *const c_void,
            inherit: i32,
            flag_bits: u32,
            env: *const c_void,
            directory: *const u16,
            start: *const InfoDeInicio,
            info: *mut ProcessInfo,
        ) -> i32;
        fn RegisterWaitForSingleObject(wait_entry: *mut Bruto, object: Bruto, ret: WaitReturn, contexto: *mut c_void, ms: u32, flag_bits: u32) -> i32;
        fn UnregisterWait(wait_entry: Bruto) -> i32;
        fn GetExitCodeProcess(process: Bruto, code: *mut u32) -> i32;
        fn TerminateProcess(process: Bruto, code: u32) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Bruto;
        fn CreateEventW(seg: *const c_void, manual: i32, initial: i32, name: *const u16) -> Bruto;
        fn WaitForMultipleObjects(n: u32, handles: *const Bruto, all_items: i32, ms: u32) -> u32;
        fn GetOverlappedResult(h: Bruto, ov: *mut OverlappedOp, n: *mut u32, wait_for: i32) -> i32;
        fn SetConsoleCtrlHandler(handler: Option<ConsoleHandler>, add: i32) -> i32;
        fn GetCurrentProcessId() -> u32;
    }

    fn system_error() -> OsError {
        // SAFETY: reads the thread's error.
        OsError::from_code(unsafe { GetLastError() } as i32)
    }

    fn wide(b: &[u8]) -> Vec<u16> {
        String::from_utf8_lossy(b).encode_utf16().collect()
    }

    /// The handles of a start, closed if it fails (or if they are not handed to
    /// anyone).
    #[derive(Default)]
    struct Stored(Vec<Bruto>);

    impl Stored {
        fn store(&mut self, hs: &[Bruto]) {
            self.0.extend(hs.iter().copied().filter(|&h| h != INVALID));
        }

        /// The handle passes to the caller.
        fn deliver(&mut self, h: Bruto) -> Bruto {
            self.0.retain(|&x| x != h);
            h
        }

        fn do_close(&mut self, h: Bruto) {
            if h != INVALID {
                self.deliver(h);
                // SAFETY: a handle of this start, closed once.
                unsafe { CloseHandle(h) };
            }
        }
    }

    impl Drop for Stored {
        fn drop(&mut self) {
            for &h in &self.0 {
                // SAFETY: handles of this start that nobody received.
                unsafe { CloseHandle(h) };
            }
        }
    }

    /// Who uses each side of a pipe.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum PipeUse {
        /// The child's input: the parent writes (overlapped), the child reads.
        ChildInput,
        /// A child's output: the child writes, the parent reads (overlapped).
        ChildOutput,
        /// The exit code pipe: the parent reads (overlapped); the write side is
        /// synchronous and does not go to the child.
        ExitCode,
        /// A signal's pipe: both sides overlapped, neither inherited.
        Signal,
    }

    /// A pipe name unique in the system (`\\.\pipe\dart-<pid>-<n>`);
    /// `FILE_FLAG_FIRST_PIPE_INSTANCE` guarantees nobody took it before.
    fn pipe_name() -> Vec<u16> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // SAFETY: a query without effects.
        let pid = unsafe { GetCurrentProcessId() };
        format!(r"\\.\pipe\dart-{pid}-{n}").encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// `CreateProcessPipe`: `[read_side, write_side]`.
    fn create_pipe(usage: PipeUse) -> IoResult<[Bruto; 2]> {
        let name = pipe_name();
        let inheritable = AtributosDeSeguranca { size: std::mem::size_of::<AtributosDeSeguranca>() as u32, descriptor: std::ptr::null_mut(), inherit: 1 };
        let parent_writes = usage == PipeUse::ChildInput;
        let opening = if parent_writes { PIPE_ACCESS_OUTBOUND } else { PIPE_ACCESS_INBOUND } | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE;
        // SAFETY: creates the pipe server with the unique name.
        let server = unsafe { CreateNamedPipeW(name.as_ptr(), opening, PIPE_REJECT_REMOTE_CLIENTS, 1, PIPE_SIZE, PIPE_SIZE, 0, std::ptr::null()) };
        if server == INVALID {
            return Err(system_error());
        }
        let (access, seg, flag_bits) = match usage {
            PipeUse::ChildInput => (GENERIC_READ | FILE_WRITE_ATTRIBUTES, &inheritable as *const _, 0),
            PipeUse::ChildOutput => (GENERIC_WRITE | FILE_READ_ATTRIBUTES, &inheritable as *const _, 0),
            PipeUse::ExitCode => (GENERIC_WRITE, std::ptr::null(), 0),
            PipeUse::Signal => (GENERIC_WRITE, std::ptr::null(), FILE_FLAG_OVERLAPPED),
        };
        // SAFETY: opens the client of the freshly created pipe.
        let client = unsafe { CreateFileW(name.as_ptr(), access, 0, seg, OPEN_EXISTING, flag_bits, 0) };
        if client == INVALID {
            let e = system_error();
            // SAFETY: the server was not handed to anyone.
            unsafe { CloseHandle(server) };
            return Err(e);
        }
        Ok(if parent_writes { [client, server] } else { [server, client] })
    }

    /// `OpenNul`: the inheritable `NUL` (the I/O of detached processes).
    fn open_nul() -> IoResult<Bruto> {
        let inheritable = AtributosDeSeguranca { size: std::mem::size_of::<AtributosDeSeguranca>() as u32, descriptor: std::ptr::null_mut(), inherit: 1 };
        let name: Vec<u16> = "NUL".encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: opens the null device.
        let h = unsafe { CreateFileW(name.as_ptr(), GENERIC_READ | GENERIC_WRITE, 0, &inheritable, OPEN_EXISTING, 0, 0) };
        if h == INVALID { Err(system_error()) } else { Ok(h) }
    }

    /// The attribute list with the inherited handles, deleted at the end.
    struct AttributeList(Vec<u64>);

    impl AttributeList {
        fn with_handles(handles: &[Bruto; 3]) -> IoResult<AttributeList> {
            let mut size = 0usize;
            // SAFETY: queries the size (fails with ERROR_INSUFFICIENT_BUFFER, as
            // expected).
            unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size) };
            let mut l = AttributeList(vec![0; size.div_ceil(8).max(1)]);
            // SAFETY: the buffer has `size` bytes, aligned to 8.
            if unsafe { InitializeProcThreadAttributeList(l.ptr(), 1, 0, &mut size) } == 0 {
                let e = system_error();
                std::mem::forget(l);
                return Err(e);
            }
            // SAFETY: `handles` lives until the end of `CreateProcessW` (it is the
            // caller's, who keeps the list for a shorter time).
            if unsafe {
                UpdateProcThreadAttribute(l.ptr(), 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, handles.as_ptr().cast(), std::mem::size_of_val(handles), std::ptr::null_mut(), std::ptr::null_mut())
            } == 0
            {
                return Err(system_error());
            }
            Ok(l)
        }

        fn ptr(&mut self) -> *mut c_void {
            self.0.as_mut_ptr().cast()
        }
    }

    impl Drop for AttributeList {
        fn drop(&mut self) {
            // SAFETY: the list was initialized.
            unsafe { DeleteProcThreadAttributeList(self.ptr()) };
        }
    }

    /// `ProcessStarter::Start`.
    pub(super) fn start_process(
        file_path: &[u8],
        arguments: &[Vec<u8>],
        directory: Option<&[u8]>,
        env: Option<&[Vec<u8>]>,
        mode: i64,
    ) -> Result<StartedProcess, OsError> {
        // The command line: the path and the arguments, already escaped by Dart,
        // separated by spaces.
        let mut line = wide(file_path);
        for a in arguments {
            line.push(u16::from(b' '));
            line.extend(wide(a));
        }
        line.push(0);
        // The environment block: `NAME=value\0…\0` (empty: two NULs).
        let block = env.map(|v| {
            let mut b = Vec::new();
            for x in v {
                b.extend(wide(x));
                b.push(0);
            }
            if v.is_empty() {
                b.push(0);
            }
            b.push(0);
            b
        });
        let dir = directory.map(|d| {
            let mut w = wide(d);
            w.push(0);
            w
        });

        let mut g = Stored::default();
        let mut input = [INVALID; 2];
        let mut output = [INVALID; 2];
        let mut error = [INVALID; 2];
        let mut end = [INVALID; 2];
        if mode != MODE_DETACHED {
            if mode_with_stdio(mode) {
                input = create_pipe(PipeUse::ChildInput)?;
                g.store(&input);
                output = create_pipe(PipeUse::ChildOutput)?;
                g.store(&output);
                error = create_pipe(PipeUse::ChildOutput)?;
                g.store(&error);
            }
            if attached_mode(mode) {
                end = create_pipe(PipeUse::ExitCode)?;
                g.store(&end);
            }
        } else {
            input[0] = open_nul()?;
            g.store(&input);
            output[1] = open_nul()?;
            g.store(&output);
            error[1] = open_nul()?;
            g.store(&error);
        }

        // SAFETY: `StartupInfoEx` is POD; zero is C's initial value.
        let mut start: StartupInfoEx = unsafe { std::mem::zeroed() };
        start.base.cb = std::mem::size_of::<StartupInfoEx>() as u32;
        let inherited = [input[0], output[1], error[1]];
        let mut list = None;
        if mode != MODE_INHERIT_STDIO {
            start.base.input = input[0];
            start.base.output = output[1];
            start.base.error = error[1];
            start.base.flag_bits = STARTF_USESTDHANDLES;
            let l = list.insert(AttributeList::with_handles(&inherited)?);
            start.attributes = l.ptr();
        }
        let mut flag_bits = EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT;
        if !attached_mode(mode) {
            flag_bits |= DETACHED_PROCESS;
        } else if mode != MODE_INHERIT_STDIO {
            flag_bits |= CREATE_NO_WINDOW;
        }
        // SAFETY: `ProcessInfo` is POD.
        let mut info: ProcessInfo = unsafe { std::mem::zeroed() };
        // SAFETY: the line is writable and NUL-terminated; the environment, the
        // directory, the attribute list and the handles live during the call.
        let ok = unsafe {
            CreateProcessW(
                std::ptr::null(),
                line.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                flag_bits,
                block.as_ref().map_or(std::ptr::null(), |b| b.as_ptr().cast()),
                dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()),
                &start.base,
                &mut info,
            )
        };
        if ok == 0 {
            return Err(system_error());
        }
        drop(list);
        // The child's sides (or `NUL`) stay only with it.
        if mode != MODE_INHERIT_STDIO {
            for h in inherited {
                g.do_close(h);
            }
        }
        // SAFETY: the child's main thread is not used.
        unsafe { CloseHandle(info.thread) };
        if attached_mode(mode) {
            register_process(info.pid, info.process, g.deliver(end[1]));
        } else {
            // SAFETY: a detached process is not tracked.
            unsafe { CloseHandle(info.process) };
        }
        let mut r = StartedProcess { pid: i64::from(info.pid), input: -1, output: -1, error: -1, end: -1 };
        if mode != MODE_DETACHED {
            let file = |g: &mut Stored, h: Bruto| descriptor_of(Handle::new(HandleKind::File, g.deliver(h)));
            if mode_with_stdio(mode) {
                r.input = file(&mut g, input[1]);
                r.output = file(&mut g, output[0]);
                r.error = file(&mut g, error[0]);
            }
            if attached_mode(mode) {
                r.end = file(&mut g, end[0]);
            }
        }
        Ok(r)
    }

    // -----------------------------------------------------------------------
    // The tracked processes (`ProcessInfoList`).

    struct LiveProcess {
        process: Bruto,
        wait_entry: Bruto,
        /// The write side of the exit code pipe.
        pipe: Bruto,
    }

    fn live_processes() -> std::sync::MutexGuard<'static, crate::hash::HashMap<u32, LiveProcess>> {
        static P: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<u32, LiveProcess>>> = std::sync::OnceLock::new();
        P.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `ProcessInfoList::AddProcess`: the system pool calls [`process_exited`]
    /// when the child exits. The lock covers the registration, so that the
    /// callback finds the process even if it has already exited.
    fn register_process(pid: u32, process: Bruto, pipe: Bruto) {
        let mut live_set = live_processes();
        let mut wait_entry = 0;
        // SAFETY: waits for the process handle, once, on the system pool.
        if unsafe { RegisterWaitForSingleObject(&mut wait_entry, process, process_exited, pid as usize as *mut c_void, INFINITE, WT_EXECUTEONLYONCE) } == 0 {
            panic!("dart:io: failed to register the wait for the process ({})", unsafe { GetLastError() });
        }
        live_set.insert(pid, LiveProcess { process, wait_entry, pipe });
    }

    /// `ExitCodeCallback`: writes `[code, negative]` into the exit pipe.
    unsafe extern "system" fn process_exited(contexto: *mut c_void, expired: u8) {
        if expired != 0 {
            return;
        }
        let pid = contexto as usize as u32;
        let Some(p) = live_processes().remove(&pid) else { return };
        // SAFETY: the wait belongs to this process; inside the callback it can
        // only be undone without blocking.
        unsafe { UnregisterWait(p.wait_entry) };
        let mut code = 0u32;
        // SAFETY: the process has ended; the handle belongs to this registration.
        unsafe { GetExitCodeProcess(p.process, &mut code) };
        let c = code as i32;
        let (value, negative) = if c < 0 { (c.wrapping_abs(), 1i32) } else { (c, 0) };
        let mut msg = [0u8; 8];
        msg[..4].copy_from_slice(&value.to_ne_bytes());
        msg[4..].copy_from_slice(&negative.to_ne_bytes());
        let mut written = 0u32;
        // SAFETY: a synchronous write to the exit code pipe (it only fails if
        // Dart already closed the other side, and then nobody waits for the code).
        unsafe {
            WriteFile(p.pipe, msg.as_ptr(), 8, &mut written, std::ptr::null_mut());
            CloseHandle(p.pipe);
            CloseHandle(p.process);
        }
    }

    /// `Process::Kill`: `TerminateProcess` with code -1 (the signal does not
    /// apply).
    pub(super) fn kill_process(pid: i64, _sinal: i64) -> bool {
        let Ok(pid) = u32::try_from(pid) else { return false };
        {
            let live_set = live_processes();
            if let Some(p) = live_set.get(&pid) {
                // SAFETY: the handle of the tracked process.
                return unsafe { TerminateProcess(p.process, u32::MAX) } != 0;
            }
        }
        // SAFETY: opens any process only to terminate it.
        let h = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
        if h == 0 {
            return false;
        }
        // SAFETY: the handle opened above, closed once.
        unsafe {
            let ok = TerminateProcess(h, u32::MAX) != 0;
            CloseHandle(h);
            ok
        }
    }

    // -----------------------------------------------------------------------
    // `Process::Wait` (`runSync`).

    /// An overlapped read with an event (bypassing the completion port: bit 0 of
    /// the event diverts it) until the end of the pipe.
    struct WaitRead {
        handle: Bruto,
        event: Bruto,
        overlapped: Box<OverlappedOp>,
        buffer: Vec<u8>,
        data: Vec<u8>,
        pending: bool,
    }

    impl WaitRead {
        /// The reader becomes the owner of `handle` (closed also if creation
        /// fails).
        fn new_area(handle: Bruto) -> IoResult<WaitRead> {
            // SAFETY: an auto-reset event, signaled so that the first read
            // starts.
            let event = unsafe { CreateEventW(std::ptr::null(), 0, 1, std::ptr::null()) };
            if event == 0 {
                let e = system_error();
                // SAFETY: the handle belongs to this reader.
                unsafe { CloseHandle(handle) };
                return Err(e);
            }
            Ok(WaitRead { handle, event, overlapped: Box::default(), buffer: vec![0; 16 * 1024], data: Vec::new(), pending: false })
        }

        /// Collects what completed and issues reads until one stays pending;
        /// `Ok(false)` at the end of the pipe.
        fn advance(&mut self) -> IoResult<bool> {
            loop {
                if self.pending {
                    let mut n = 0u32;
                    // SAFETY: the read issued with this `OVERLAPPED`.
                    if unsafe { GetOverlappedResult(self.handle, &mut *self.overlapped, &mut n, 0) } == 0 {
                        return match unsafe { GetLastError() } {
                            ERROR_IO_INCOMPLETE => Ok(true),
                            ERROR_BROKEN_PIPE => Ok(false),
                            c => Err(OsError::from_code(c as i32)),
                        };
                    }
                    self.pending = false;
                    self.data.extend_from_slice(&self.buffer[..n as usize]);
                }
                *self.overlapped = OverlappedOp::with_event(self.event | 1);
                // SAFETY: the buffer and the `OVERLAPPED` live until completion
                // (the read is collected or the handle closed before they are
                // released).
                let ok = unsafe { ReadFile(self.handle, self.buffer.as_mut_ptr(), self.buffer.len() as u32, std::ptr::null_mut(), &mut *self.overlapped) } != 0;
                self.pending = true;
                if !ok {
                    match unsafe { GetLastError() } {
                        ERROR_IO_PENDING => return Ok(true),
                        ERROR_BROKEN_PIPE => {
                            self.pending = false;
                            return Ok(false);
                        }
                        c => {
                            self.pending = false;
                            return Err(OsError::from_code(c as i32));
                        }
                    }
                }
            }
        }
    }

    impl Drop for WaitRead {
        fn drop(&mut self) {
            // SAFETY: closing the handle aborts the pending read; the event
            // belongs to this reader.
            unsafe {
                CloseHandle(self.handle);
                CloseHandle(self.event);
            }
        }
    }

    /// Takes the handle out of a pipe handle object: it no longer closes it.
    fn take_handle(fd: i64) -> IoResult<Bruto> {
        let m = handle_of(fd).ok_or_else(invalid_handle)?;
        m.estado().take_handle();
        Ok(m.raw)
    }

    /// `Process::Wait`: closes the input, reads the output and the error to the
    /// end and the code from the exit pipe; `(code, output, error)`.
    pub(super) fn wait_process(input: i64, output: i64, error: i64, end: i64) -> IoResult<(i64, Vec<u8>, Vec<u8>)> {
        // SAFETY: closes the handle of the child's input.
        unsafe { CloseHandle(take_handle(input)?) };
        let mut readers = Vec::with_capacity(3);
        for fd in [output, error, end] {
            readers.push(WaitRead::new_area(take_handle(fd)?)?);
        }
        let mut live_set: Vec<usize> = vec![0, 1, 2];
        while !live_set.is_empty() {
            let events: Vec<Bruto> = live_set.iter().map(|&i| readers[i].event).collect();
            // SAFETY: the events of the live readers.
            let r = unsafe { WaitForMultipleObjects(events.len() as u32, events.as_ptr(), 0, INFINITE) };
            let k = r.wrapping_sub(WAIT_OBJECT_0) as usize;
            if k >= live_set.len() {
                return Err(system_error());
            }
            if !readers[live_set[k]].advance()? {
                live_set.remove(k);
            }
        }
        let code = &readers[2].data;
        if code.len() != 8 {
            return Err(OsError { code: -1, message: "Failed to read the process exit code".to_string() });
        }
        let value = i64::from(i32::from_ne_bytes(code[..4].try_into().expect("4 bytes")));
        let negative = i32::from_ne_bytes(code[4..].try_into().expect("4 bytes"));
        let mut l = readers.into_iter();
        let out = std::mem::take(&mut l.next().expect("output").data);
        let err = std::mem::take(&mut l.next().expect("error").data);
        Ok((if negative != 0 { -value } else { value }, out, err))
    }

    // -----------------------------------------------------------------------
    // Signals (console events).

    struct SignalSubscription {
        event: u32,
        /// The side the handler writes.
        write_side: Arc<Handle>,
        /// The descriptor of the side Dart reads.
        read_side: i64,
    }

    fn subscriptions() -> std::sync::MutexGuard<'static, Vec<SignalSubscription>> {
        static I: std::sync::Mutex<Vec<SignalSubscription>> = std::sync::Mutex::new(Vec::new());
        I.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `GetWinSignal`: SIGHUP is the console being closed; SIGINT, Ctrl+C.
    fn console_event(sig_num: i64) -> Option<u32> {
        match sig_num {
            1 => Some(CTRL_CLOSE_EVENT),
            2 => Some(CTRL_C_EVENT),
            _ => None,
        }
    }

    /// `SignalHandler`: one byte into the pipe of each subscription of the event.
    unsafe extern "system" fn handle_console(event: u32) -> i32 {
        let l = subscriptions();
        let mut handled = false;
        for i in l.iter().filter(|i| i.event == event) {
            let _ = write_to_handle(&i.write_side, &[0]);
            handled = true;
        }
        i32::from(handled)
    }

    /// `Process::SetSignalHandler`: the descriptor of the pipe that Dart reads.
    pub(super) fn subscribe_signal(sig_num: i64) -> IoResult<i64> {
        let Some(event) = console_event(sig_num) else {
            return Err(OsError::from_code(ERROR_NOT_SUPPORTED));
        };
        let [read_side, write_side] = create_pipe(PipeUse::Signal)?;
        let write_side = Handle::new(HandleKind::File, write_side);
        let read_side = Handle::new(HandleKind::File, read_side);
        let mut l = subscriptions();
        // SAFETY: installs the console handler (once for all subscriptions).
        if l.is_empty() && unsafe { SetConsoleCtrlHandler(Some(handle_console), 1) } == 0 {
            return Err(system_error());
        }
        let fd = descriptor_of(read_side);
        l.push(SignalSubscription { event, write_side, read_side: fd });
        Ok(fd)
    }

    fn remove_subscriptions(filter_obj: impl Fn(&SignalSubscription) -> bool) {
        let mut l = subscriptions();
        let before = l.len();
        l.retain(|i| !filter_obj(i));
        if before > 0 && l.is_empty() {
            // SAFETY: removes the handler installed by `subscribe_signal`.
            unsafe { SetConsoleCtrlHandler(Some(handle_console), 0) };
        }
    }

    /// `Process::ClearSignalHandler`.
    pub(super) fn cancel_signal(sig_num: i64) {
        if let Some(event) = console_event(sig_num) {
            remove_subscriptions(|i| i.event == event);
        }
    }

    /// `Process::ClearSignalHandlerByFd`: the signal's socket was closed.
    pub(super) fn clear_signal_by_descriptor(fd: i64) {
        remove_subscriptions(|i| i.read_side == fd);
    }
}

#[cfg(windows)]
use windows_processes::*;
