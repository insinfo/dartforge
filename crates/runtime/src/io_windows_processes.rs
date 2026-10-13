// Runtime nativo: os processos do `dart:io` no Windows (`process_win.cc`
// da VM).
//
// O filho nasce de `CreateProcessW` com a linha de comando que o Dart já
// escapou (`_windowsArgumentEscape`), o ambiente em UTF-16 e, fora do modo
// `inheritStdio`, só os três handles de E/S herdados
// (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`: nenhum outro handle do processo
// vaza para o filho). A E/S padrão do filho são pipes nomeados: o lado do
// pai é sobreposto e vira um [`Handle`] na porta de conclusão; o do
// filho é síncrono, como um programa espera. O código de saída chega pelo
// pool de threads do sistema (`RegisterWaitForSingleObject`), que o escreve
// (`[código, negative]`, dois `int32`) no pipe de saída do processo.
//
// Os sinais são os eventos de console (`SetConsoleCtrlHandler`): SIGINT é
// o Ctrl+C e SIGHUP o fechamento da janela.

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
    /// O buffer de cada sentido dos pipes (o do `libuv`).
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
        // SAFETY: lê o erro da thread.
        OsError::from_code(unsafe { GetLastError() } as i32)
    }

    fn wide(b: &[u8]) -> Vec<u16> {
        String::from_utf8_lossy(b).encode_utf16().collect()
    }

    /// Os handles de um início, fechados se ele falha (ou se não forem
    /// entregues a ninguém).
    #[derive(Default)]
    struct Stored(Vec<Bruto>);

    impl Stored {
        fn store(&mut self, hs: &[Bruto]) {
            self.0.extend(hs.iter().copied().filter(|&h| h != INVALID));
        }

        /// O handle passa a quem chama.
        fn deliver(&mut self, h: Bruto) -> Bruto {
            self.0.retain(|&x| x != h);
            h
        }

        fn do_close(&mut self, h: Bruto) {
            if h != INVALID {
                self.deliver(h);
                // SAFETY: handle deste início, fechado uma vez.
                unsafe { CloseHandle(h) };
            }
        }
    }

    impl Drop for Stored {
        fn drop(&mut self) {
            for &h in &self.0 {
                // SAFETY: handles deste início que ninguém recebeu.
                unsafe { CloseHandle(h) };
            }
        }
    }

    /// Quem usa cada lado de um pipe.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum PipeUse {
        /// A entrada do filho: o pai escreve (sobreposto), o filho lê.
        ChildInput,
        /// Uma saída do filho: o filho escreve, o pai lê (sobreposto).
        ChildOutput,
        /// O pipe do código de saída: o pai lê (sobreposto); a escrita é
        /// síncrona e não vai ao filho.
        ExitCode,
        /// O pipe de um sinal: os dois lados sobrepostos, nenhum herdado.
        Signal,
    }

    /// Um nome de pipe único no sistema (`\\.\pipe\dart-<pid>-<n>`); o
    /// `FILE_FLAG_FIRST_PIPE_INSTANCE` garante que ninguém o tomou antes.
    fn pipe_name() -> Vec<u16> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // SAFETY: consulta sem efeitos.
        let pid = unsafe { GetCurrentProcessId() };
        format!(r"\\.\pipe\dart-{pid}-{n}").encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// `CreateProcessPipe`: `[read_side, write_side]`.
    fn create_pipe(usage: PipeUse) -> IoResult<[Bruto; 2]> {
        let name = pipe_name();
        let inheritable = AtributosDeSeguranca { size: std::mem::size_of::<AtributosDeSeguranca>() as u32, descriptor: std::ptr::null_mut(), inherit: 1 };
        let parent_writes = usage == PipeUse::ChildInput;
        let opening = if parent_writes { PIPE_ACCESS_OUTBOUND } else { PIPE_ACCESS_INBOUND } | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE;
        // SAFETY: cria o servidor do pipe com o nome único.
        let tls_server = unsafe { CreateNamedPipeW(name.as_ptr(), opening, PIPE_REJECT_REMOTE_CLIENTS, 1, PIPE_SIZE, PIPE_SIZE, 0, std::ptr::null()) };
        if tls_server == INVALID {
            return Err(system_error());
        }
        let (access, seg, flag_bits) = match usage {
            PipeUse::ChildInput => (GENERIC_READ | FILE_WRITE_ATTRIBUTES, &inheritable as *const _, 0),
            PipeUse::ChildOutput => (GENERIC_WRITE | FILE_READ_ATTRIBUTES, &inheritable as *const _, 0),
            PipeUse::ExitCode => (GENERIC_WRITE, std::ptr::null(), 0),
            PipeUse::Signal => (GENERIC_WRITE, std::ptr::null(), FILE_FLAG_OVERLAPPED),
        };
        // SAFETY: abre o cliente do pipe recém-criado.
        let client = unsafe { CreateFileW(name.as_ptr(), access, 0, seg, OPEN_EXISTING, flag_bits, 0) };
        if client == INVALID {
            let e = system_error();
            // SAFETY: o servidor não foi entregue a ninguém.
            unsafe { CloseHandle(tls_server) };
            return Err(e);
        }
        Ok(if parent_writes { [client, tls_server] } else { [tls_server, client] })
    }

    /// `OpenNul`: o `NUL` herdável (a E/S dos processos destacados).
    fn open_nul() -> IoResult<Bruto> {
        let inheritable = AtributosDeSeguranca { size: std::mem::size_of::<AtributosDeSeguranca>() as u32, descriptor: std::ptr::null_mut(), inherit: 1 };
        let name: Vec<u16> = "NUL".encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: abre o dispositivo nulo.
        let h = unsafe { CreateFileW(name.as_ptr(), GENERIC_READ | GENERIC_WRITE, 0, &inheritable, OPEN_EXISTING, 0, 0) };
        if h == INVALID { Err(system_error()) } else { Ok(h) }
    }

    /// A lista de atributos com os handles herdados, apagada no fim.
    struct AttributeList(Vec<u64>);

    impl AttributeList {
        fn with_handles(handles: &[Bruto; 3]) -> IoResult<AttributeList> {
            let mut size = 0usize;
            // SAFETY: consulta o tamanho (falha com
            // ERROR_INSUFFICIENT_BUFFER, esperado).
            unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size) };
            let mut l = AttributeList(vec![0; size.div_ceil(8).max(1)]);
            // SAFETY: o buffer tem `size` bytes, alinhado a 8.
            if unsafe { InitializeProcThreadAttributeList(l.ptr(), 1, 0, &mut size) } == 0 {
                let e = system_error();
                std::mem::forget(l);
                return Err(e);
            }
            // SAFETY: `handles` vive até o fim do `CreateProcessW` (é de
            // quem chama, que mantém a lista por menos tempo).
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
            // SAFETY: a lista foi iniciada.
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
        // A linha de comando: o caminho e os argumentos, já escapados pelo
        // Dart, separados por espaço.
        let mut line = wide(file_path);
        for a in arguments {
            line.push(u16::from(b' '));
            line.extend(wide(a));
        }
        line.push(0);
        // O bloco de ambiente: `NOME=valor\0…\0` (vazio: dois NULs).
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

        // SAFETY: `StartupInfoEx` é POD; zero é o valor inicial do C.
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
        // SAFETY: `ProcessInfo` é POD.
        let mut info: ProcessInfo = unsafe { std::mem::zeroed() };
        // SAFETY: a linha é gravável e terminada em NUL; o ambiente, o
        // diretório, a lista de atributos e os handles vivem durante a
        // chamada.
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
        // Os lados do filho (ou o `NUL`) ficam só com ele.
        if mode != MODE_INHERIT_STDIO {
            for h in inherited {
                g.do_close(h);
            }
        }
        // SAFETY: a thread principal do filho não é usada.
        unsafe { CloseHandle(info.thread) };
        if attached_mode(mode) {
            register_process(info.pid, info.process, g.deliver(end[1]));
        } else {
            // SAFETY: o processo destacado não é acompanhado.
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
    // Os processos acompanhados (`ProcessInfoList`).

    struct LiveProcess {
        process: Bruto,
        wait_entry: Bruto,
        /// O lado de escrita do pipe do código de saída.
        pipe: Bruto,
    }

    fn live_processes() -> std::sync::MutexGuard<'static, crate::hash::HashMap<u32, LiveProcess>> {
        static P: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<u32, LiveProcess>>> = std::sync::OnceLock::new();
        P.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `ProcessInfoList::AddProcess`: o pool do sistema chama
    /// [`process_exited`] quando o filho sai. A trava cobre o registro,
    /// para o retorno achar o processo mesmo se ele já saiu.
    fn register_process(pid: u32, process: Bruto, pipe: Bruto) {
        let mut live_set = live_processes();
        let mut wait_entry = 0;
        // SAFETY: espera o handle do processo, uma vez, no pool do sistema.
        if unsafe { RegisterWaitForSingleObject(&mut wait_entry, process, process_exited, pid as usize as *mut c_void, INFINITE, WT_EXECUTEONLYONCE) } == 0 {
            panic!("dart:io: falha ao registrar a espera do processo ({})", unsafe { GetLastError() });
        }
        live_set.insert(pid, LiveProcess { process, wait_entry, pipe });
    }

    /// `ExitCodeCallback`: escreve `[código, negative]` no pipe de saída.
    unsafe extern "system" fn process_exited(contexto: *mut c_void, expired: u8) {
        if expired != 0 {
            return;
        }
        let pid = contexto as usize as u32;
        let Some(p) = live_processes().remove(&pid) else { return };
        // SAFETY: a espera é deste processo; dentro do retorno ela só pode
        // ser desfeita sem bloquear.
        unsafe { UnregisterWait(p.wait_entry) };
        let mut code = 0u32;
        // SAFETY: o processo terminou; o handle é deste registro.
        unsafe { GetExitCodeProcess(p.process, &mut code) };
        let c = code as i32;
        let (value, negative) = if c < 0 { (c.wrapping_abs(), 1i32) } else { (c, 0) };
        let mut msg = [0u8; 8];
        msg[..4].copy_from_slice(&value.to_ne_bytes());
        msg[4..].copy_from_slice(&negative.to_ne_bytes());
        let mut written = 0u32;
        // SAFETY: escrita síncrona no pipe do código de saída (falha só se
        // o Dart já fechou o outro lado, e aí ninguém espera o código).
        unsafe {
            WriteFile(p.pipe, msg.as_ptr(), 8, &mut written, std::ptr::null_mut());
            CloseHandle(p.pipe);
            CloseHandle(p.process);
        }
    }

    /// `Process::Kill`: `TerminateProcess` com o código -1 (o sinal não se
    /// aplica).
    pub(super) fn kill_process(pid: i64, _sinal: i64) -> bool {
        let Ok(pid) = u32::try_from(pid) else { return false };
        {
            let live_set = live_processes();
            if let Some(p) = live_set.get(&pid) {
                // SAFETY: o handle do processo acompanhado.
                return unsafe { TerminateProcess(p.process, u32::MAX) } != 0;
            }
        }
        // SAFETY: abre um processo qualquer só para terminá-lo.
        let h = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
        if h == 0 {
            return false;
        }
        // SAFETY: o handle aberto acima, fechado uma vez.
        unsafe {
            let ok = TerminateProcess(h, u32::MAX) != 0;
            CloseHandle(h);
            ok
        }
    }

    // -----------------------------------------------------------------------
    // `Process::Wait` (o `runSync`).

    /// Uma leitura sobreposta com evento (sem passar pela porta de
    /// conclusão: o bit 0 do evento a desvia) até o fim do pipe.
    struct WaitRead {
        handle: Bruto,
        event: Bruto,
        overlapped: Box<OverlappedOp>,
        buffer: Vec<u8>,
        data: Vec<u8>,
        pending: bool,
    }

    impl WaitRead {
        /// O leitor passa a ser dono de `handle` (fechado também se a
        /// criação falha).
        fn new_area(handle: Bruto) -> IoResult<WaitRead> {
            // SAFETY: um evento automático, sinalizado para a primeira
            // leitura começar.
            let event = unsafe { CreateEventW(std::ptr::null(), 0, 1, std::ptr::null()) };
            if event == 0 {
                let e = system_error();
                // SAFETY: o handle é deste leitor.
                unsafe { CloseHandle(handle) };
                return Err(e);
            }
            Ok(WaitRead { handle, event, overlapped: Box::default(), buffer: vec![0; 16 * 1024], data: Vec::new(), pending: false })
        }

        /// Recolhe o que concluiu e emite leituras até uma ficar pendente;
        /// `Ok(false)` no fim do pipe.
        fn advance(&mut self) -> IoResult<bool> {
            loop {
                if self.pending {
                    let mut n = 0u32;
                    // SAFETY: a leitura emitida com este `OVERLAPPED`.
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
                // SAFETY: o buffer e o `OVERLAPPED` vivem até a conclusão
                // (a leitura é recolhida ou o handle fechado antes de
                // soltá-los).
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
            // SAFETY: fechar o handle aborta a leitura pendente; o evento é
            // deste leitor.
            unsafe {
                CloseHandle(self.handle);
                CloseHandle(self.event);
            }
        }
    }

    /// Toma o handle de um manipulador de pipe: o manipulador não o fecha
    /// mais.
    fn take_handle(fd: i64) -> IoResult<Bruto> {
        let m = handle_of(fd).ok_or_else(invalid_handle)?;
        m.estado().take_handle();
        Ok(m.raw)
    }

    /// `Process::Wait`: fecha a entrada, lê a saída e o erro até o fim e o
    /// código do pipe de saída; `(código, saída, error)`.
    pub(super) fn wait_process(input: i64, output: i64, error: i64, end: i64) -> IoResult<(i64, Vec<u8>, Vec<u8>)> {
        // SAFETY: fecha o handle da entrada do filho.
        unsafe { CloseHandle(take_handle(input)?) };
        let mut readers = Vec::with_capacity(3);
        for fd in [output, error, end] {
            readers.push(WaitRead::new_area(take_handle(fd)?)?);
        }
        let mut live_set: Vec<usize> = vec![0, 1, 2];
        while !live_set.is_empty() {
            let events: Vec<Bruto> = live_set.iter().map(|&i| readers[i].event).collect();
            // SAFETY: os eventos dos leitores vivos.
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
        let out = std::mem::take(&mut l.next().expect("saída").data);
        let err = std::mem::take(&mut l.next().expect("erro").data);
        Ok((if negative != 0 { -value } else { value }, out, err))
    }

    // -----------------------------------------------------------------------
    // Sinais (eventos de console).

    struct SignalSubscription {
        event: u32,
        /// O lado que o tratador escreve.
        write_side: Arc<Handle>,
        /// O descritor do lado que o Dart lê.
        read_side: i64,
    }

    fn subscriptions() -> std::sync::MutexGuard<'static, Vec<SignalSubscription>> {
        static I: std::sync::Mutex<Vec<SignalSubscription>> = std::sync::Mutex::new(Vec::new());
        I.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `GetWinSignal`: SIGHUP é o fechamento do console; SIGINT, o Ctrl+C.
    fn console_event(sig_num: i64) -> Option<u32> {
        match sig_num {
            1 => Some(CTRL_CLOSE_EVENT),
            2 => Some(CTRL_C_EVENT),
            _ => None,
        }
    }

    /// `SignalHandler`: um byte no pipe de cada inscrição do evento.
    unsafe extern "system" fn handle_console(event: u32) -> i32 {
        let l = subscriptions();
        let mut handled = false;
        for i in l.iter().filter(|i| i.event == event) {
            let _ = write_to_handle(&i.write_side, &[0]);
            handled = true;
        }
        i32::from(handled)
    }

    /// `Process::SetSignalHandler`: o descritor do pipe que o Dart lê.
    pub(super) fn subscribe_signal(sig_num: i64) -> IoResult<i64> {
        let Some(event) = console_event(sig_num) else {
            return Err(OsError::from_code(ERROR_NOT_SUPPORTED));
        };
        let [read_side, write_side] = create_pipe(PipeUse::Signal)?;
        let write_side = Handle::new(HandleKind::File, write_side);
        let read_side = Handle::new(HandleKind::File, read_side);
        let mut l = subscriptions();
        // SAFETY: instala o tratador de console (uma vez para todas as
        // inscrições).
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
            // SAFETY: remove o tratador instalado por `subscribe_signal`.
            unsafe { SetConsoleCtrlHandler(Some(handle_console), 0) };
        }
    }

    /// `Process::ClearSignalHandler`.
    pub(super) fn cancel_signal(sig_num: i64) {
        if let Some(event) = console_event(sig_num) {
            remove_subscriptions(|i| i.event == event);
        }
    }

    /// `Process::ClearSignalHandlerByFd`: o soquete do sinal fechou.
    pub(super) fn clear_signal_by_descriptor(fd: i64) {
        remove_subscriptions(|i| i.read_side == fd);
    }
}

#[cfg(windows)]
use windows_processes::*;
