// Runtime nativo: o manipulador de eventos do `dart:io` da VM
// (`runtime/bin/eventhandler*.cc`) e o objeto de soquete (`bin/socket.h`).
//
// Um `_NativeSocket` do Dart (soquete TCP, pipe de processo, entrada padrão)
// guarda no campo nativo um [`NativeSocket`]: o descritor e a porta Dart
// que recebe os eventos dele, com contagem de referências. O Dart pede ao
// manipulador, por `EventHandler_SendData`, para vigiar o descritor (uma
// máscara de leitura/escrita), devolver fichas ou fechar; o manipulador é
// uma thread com epoll (Linux), kqueue (macOS) ou uma porta de conclusão de
// E/S (Windows, em `io_windows_eventos.rs`) que posta na porta do
// soquete a máscara dos eventos prontos (`kInEvent`, `kOutEvent`,
// `kCloseEvent`, `kErrorEvent`, `kDestroyedEvent`) — como a VM, inclusive
// o controle de fluxo por fichas (`TokenCounter`) e os soquetes de escuta
// compartilhados entre portas (`DescriptorInfoMultiple`).
//
// Os timers do Dart não passam por aqui: o laço de eventos do isolado os
// atende (`eventos.rs`), e a sobreposição de `dart:async` não usa o
// `_EventHandler` para eles.

/// `MessageFlags` de `eventhandler.h`.
const EVENT_IN: i64 = 0;
const EVENT_OUT: i64 = 1;
const EVENT_ERROR: i64 = 2;
const EVENT_CLOSE: i64 = 3;
const EVENT_DESTROYED: i64 = 4;
const COMMAND_CLOSE: i64 = 8;
const COMMAND_SHUTDOWN_READ: i64 = 9;
const COMMAND_SHUTDOWN_WRITE: i64 = 10;
const COMMAND_RETURN_TOKENS: i64 = 11;
const COMMAND_MASK: i64 = 12;
const LISTENING_SOCKET: i64 = 16;
const SIGNAL_SOCKET: i64 = 18;

const COMMANDS_MASK: i64 = (1 << COMMAND_CLOSE)
    | (1 << COMMAND_SHUTDOWN_READ)
    | (1 << COMMAND_SHUTDOWN_WRITE)
    | (1 << COMMAND_RETURN_TOKENS)
    | (1 << COMMAND_MASK);
const EVENTS_MASK: i64 = (1 << EVENT_IN)
    | (1 << EVENT_OUT)
    | (1 << EVENT_ERROR)
    | (1 << EVENT_CLOSE)
    | (1 << EVENT_DESTROYED);

fn is_command(data: i64, bit: i64) -> bool {
    data & COMMANDS_MASK == 1 << bit
}

// ---------------------------------------------------------------------------
// O soquete nativo.

/// Um descritor que o Dart usa como `_NativeSocket` (o `Socket` da VM).
struct NativeSocket {
    descriptor: std::sync::atomic::AtomicI64,
    /// A porta Dart dos eventos (a `eventPort` do `_NativeSocket`).
    port: std::sync::atomic::AtomicI64,
}

impl NativeSocket {
    /// Cria o soquete com uma referência e devolve o ponteiro dela.
    fn new(descriptor: i64) -> i64 {
        std::sync::Arc::into_raw(std::sync::Arc::new(NativeSocket {
            descriptor: std::sync::atomic::AtomicI64::new(descriptor),
            port: std::sync::atomic::AtomicI64::new(0),
        })) as i64
    }

    /// # Safety
    /// `p` veio de [`NativeSocket::new`] e a referência ainda existe.
    unsafe fn de<'a>(p: i64) -> &'a NativeSocket {
        // SAFETY: garantido por quem chama.
        unsafe { &*(p as *const NativeSocket) }
    }

    fn retain_ref(p: i64) {
        // SAFETY: `p` é um ponteiro vivo de `new`.
        unsafe { std::sync::Arc::increment_strong_count(p as *const NativeSocket) };
    }

    fn free(p: i64) {
        // SAFETY: `p` é um ponteiro vivo de `new`, com esta referência.
        unsafe { std::sync::Arc::decrement_strong_count(p as *const NativeSocket) };
    }

    fn descriptor(&self) -> i64 {
        self.descriptor.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Fecha o recurso do sistema (o descritor, no Unix; a referência do
    /// `Handle`, no Windows, cujo fechamento o manipulador já fez).
    fn close_descriptor(&self) {
        let d = self.descriptor.swap(-1, std::sync::atomic::Ordering::AcqRel);
        if d >= 0 {
            close_socket_descriptor(d);
        }
    }

    /// `Socket::CloseFd`: o objeto deixa o descritor sem fechá-lo no Unix
    /// (quem fecha é o manipulador, ou ninguém: a entrada padrão); no
    /// Windows solta a referência do `Handle`.
    fn release_descriptor(&self) {
        #[cfg(unix)]
        self.descriptor.store(-1, std::sync::atomic::Ordering::Release);
        #[cfg(windows)]
        self.close_descriptor();
    }
}

/// No Windows cada objeto é dono de uma referência do `Handle` (o
/// `descriptor`); ela é solta com o objeto, se ainda não foi.
#[cfg(windows)]
impl Drop for NativeSocket {
    fn drop(&mut self) {
        let d = *self.descriptor.get_mut();
        if d >= 0 {
            close_socket_descriptor(d);
        }
    }
}

#[cfg(unix)]
fn close_socket_descriptor(d: i64) {
    unsafe extern "C" {
        fn close(fd: i32) -> i32;
    }
    // SAFETY: `d` é um descritor deste soquete, fechado uma vez.
    unsafe { close(d as i32) };
}

/// Outra referência ao descritor de um soquete de escuta compartilhado
/// (no Unix o descritor é o mesmo, sem contagem).
#[cfg(unix)]
fn retain_descriptor(d: i64) -> i64 {
    d
}

/// O soquete da entrada padrão (`SocketBase::GetStdioHandle`): o próprio
/// descritor no Unix.
#[cfg(unix)]
fn default_handle(num: i64) -> i64 {
    default_descriptor(num)
}

/// Os finalizadores do `_NativeSocket` coletado sem `close` (os
/// `*SocketFinalizer` de `socket.cc`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum SocketFinalizer {
    Normal,
    Listen,
    Stdio,
    Signal,
}

fn finalize_normal_socket(pair: usize) {
    close_collected_socket(pair as i64, 1 << COMMAND_CLOSE);
}

fn finalize_listening_socket(pair: usize) {
    close_collected_socket(pair as i64, (1 << LISTENING_SOCKET) | (1 << COMMAND_CLOSE));
}

fn finalize_signal_socket(pair: usize) {
    close_collected_socket(pair as i64, (1 << SIGNAL_SOCKET) | (1 << COMMAND_CLOSE));
}

fn finalize_stdio_socket(pair: usize) {
    // SAFETY: a referência do objeto ainda existe; é solta aqui.
    unsafe { NativeSocket::de(pair as i64) }.release_descriptor();
    NativeSocket::free(pair as i64);
}

/// Envia o comando de fechar de um soquete coletado ao manipulador: a
/// referência do objeto passa ao comando, que a solta.
fn close_collected_socket(p: i64, data: i64) {
    // SAFETY: a referência do objeto coletado ainda existe.
    let port = unsafe { NativeSocket::de(p) }.port.load(std::sync::atomic::Ordering::Acquire);
    event_handler().enviar(EventCommand { sock: p, port, data });
}

/// `Socket::SetSocketIdNativeField`: cria o soquete de `descriptor` no campo
/// nativo de `object`, com o finalizador.
fn set_socket_on_object(object: i64, descriptor: i64, finalizer: SocketFinalizer) {
    reuse_socket_on_object(object, NativeSocket::new(descriptor), finalizer);
}

/// `Socket::ReuseSocketIdNativeField`.
fn reuse_socket_on_object(object: i64, p: i64, finalizer: SocketFinalizer) {
    write_native_field(object, p);
    let f: fn(usize) = match finalizer {
        SocketFinalizer::Normal => finalize_normal_socket,
        SocketFinalizer::Listen => finalize_listening_socket,
        SocketFinalizer::Stdio => finalize_stdio_socket,
        SocketFinalizer::Signal => finalize_signal_socket,
    };
    attach_finalizer(object, f, p as usize);
}

/// `Socket::GetSocketIdNativeField`: o soquete do objeto, ou lança o erro
/// interno ("No native peer").
fn object_socket<'a>(object: i64) -> Option<&'a NativeSocket> {
    let p = native_field(object);
    if p == 0 {
        throw_internal_error("No native peer");
        return None;
    }
    // SAFETY: a referência do objeto vive até a coleta dele.
    Some(unsafe { NativeSocket::de(p) })
}

// ---------------------------------------------------------------------------
// As fichas e os interessados de um descritor (`DescriptorInfo*Mixin`).

/// O que o manipulador sabe de um descritor.
enum DescriptorInfo {
    /// Um soquete comum: uma porta, uma máscara e 16 fichas.
    Single { port: i64, mask: i64, tokens: i64 },
    /// Um soquete de escuta compartilhado: cada porta com 4 fichas e se
    /// quer ler; as prontas se revezam (`active_readers_`).
    Multiple { ports: Vec<PortEntry>, active: std::collections::VecDeque<i64> },
}

struct PortEntry {
    port: i64,
    reading: bool,
    tokens: i64,
}

impl PortEntry {
    fn ready_one(&self) -> bool {
        self.tokens > 0 && self.reading
    }
}

const SINGLE_TOKENS: i64 = 16;
const MULTIPLE_TOKENS: i64 = 4;

impl DescriptorInfo {
    fn new(listening: bool) -> DescriptorInfo {
        if listening {
            DescriptorInfo::Multiple { ports: Vec::new(), active: std::collections::VecDeque::new() }
        } else {
            DescriptorInfo::Single { port: 0, mask: 0, tokens: SINGLE_TOKENS }
        }
    }

    fn listening(&self) -> bool {
        matches!(self, DescriptorInfo::Multiple { .. })
    }

    fn set_port_and_mask(&mut self, p: i64, m: i64) {
        match self {
            DescriptorInfo::Single { port, mask, .. } => {
                *port = p;
                *mask = m;
            }
            DescriptorInfo::Multiple { ports, active } => {
                let reading = m == 1 << EVENT_IN;
                match ports.iter_mut().find(|e| e.port == p) {
                    None => {
                        let e = PortEntry { port: p, reading, tokens: MULTIPLE_TOKENS };
                        if e.ready_one() {
                            active.push_back(p);
                        }
                        ports.push(e);
                    }
                    Some(e) => {
                        let was_ready = e.ready_one();
                        e.reading = reading;
                        let is_ready = e.ready_one();
                        if was_ready && !is_ready {
                            active.retain(|&x| x != p);
                        } else if !was_ready && is_ready {
                            active.push_back(p);
                        }
                    }
                }
            }
        }
    }

    fn remove_port(&mut self, p: i64) {
        match self {
            DescriptorInfo::Single { port, mask, .. } => {
                *port = 0;
                *mask = 0;
            }
            DescriptorInfo::Multiple { ports, active } => {
                ports.retain(|e| e.port != p);
                active.retain(|&x| x != p);
            }
        }
    }

    /// A porta a avisar de um evento de E/S (e uma ficha a menos).
    fn next_port(&mut self) -> i64 {
        match self {
            DescriptorInfo::Single { port, tokens, .. } => {
                *tokens -= 1;
                *port
            }
            DescriptorInfo::Multiple { ports, active } => {
                let Some(p) = active.pop_front() else { return 0 };
                if let Some(e) = ports.iter_mut().find(|e| e.port == p) {
                    e.tokens -= 1;
                    if e.ready_one() {
                        active.push_back(p);
                    }
                }
                p
            }
        }
    }

    /// Avisa todas as portas (fechamento, erro, destruição).
    fn notify_all_waiters(&mut self, events: i64) {
        match self {
            DescriptorInfo::Single { port, tokens, .. } => {
                if *port != 0 {
                    post_event(*port, events);
                }
                *tokens -= 1;
            }
            DescriptorInfo::Multiple { ports, active } => {
                for e in ports.iter_mut() {
                    post_event(e.port, events);
                    let was_ready = e.ready_one();
                    e.tokens -= 1;
                    if was_ready && !e.ready_one() {
                        let p = e.port;
                        active.retain(|&x| x != p);
                    }
                }
            }
        }
    }

    fn return_tokens(&mut self, p: i64, n: i64) {
        match self {
            DescriptorInfo::Single { tokens, .. } => *tokens += n,
            DescriptorInfo::Multiple { ports, active } => {
                if let Some(e) = ports.iter_mut().find(|e| e.port == p) {
                    let was_ready = e.ready_one();
                    e.tokens += n;
                    if !was_ready && e.ready_one() {
                        active.push_back(p);
                    }
                }
            }
        }
    }

    /// A máscara a vigiar agora (vazia sem fichas).
    /// `RemoveAllPorts`.
    fn remove_all(&mut self) {
        match self {
            DescriptorInfo::Single { port, mask, .. } => {
                *port = 0;
                *mask = 0;
            }
            DescriptorInfo::Multiple { ports, active } => {
                ports.clear();
                active.clear();
            }
        }
    }

    fn mask(&self) -> i64 {
        match self {
            DescriptorInfo::Single { mask, tokens, .. } => {
                if *tokens <= 0 { 0 } else { *mask }
            }
            DescriptorInfo::Multiple { active, .. } => {
                if active.is_empty() { 0 } else { 1 << EVENT_IN }
            }
        }
    }
}

/// `DARTFORGE_IO_RASTRO=1`: uma linha no stderr por evento da sondagem, por
/// pedido ao manipulador e por mudança do que a sondagem vigia (o
/// diagnóstico dos defeitos de `dart:io` que só aparecem num sistema).
#[cfg(unix)]
fn io_trace() -> bool {
    static R: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *R.get_or_init(|| std::env::var("DARTFORGE_IO_RASTRO").is_ok_and(|v| v == "1"))
}

/// `DartUtils::PostInt32`: a máscara de eventos na porta.
fn post_event(port: i64, events: i64) {
    if port != 0 {
        post(port, Graph::from_int(events));
    }
}

// ---------------------------------------------------------------------------
// O manipulador.

/// Um pedido ao manipulador (o `InterruptMessage` da VM): o soquete (com a
/// referência que o pedido carrega), a porta e os dados.
struct EventCommand {
    sock: i64,
    port: i64,
    data: i64,
}

/// A thread do manipulador e a fila de pedidos dela; um byte no pipe de
/// interrupção a acorda. No Windows o pedido vai pela porta de conclusão
/// (`io_windows_eventos.rs`).
struct EventHandler {
    #[cfg(unix)]
    requests: std::sync::Mutex<Vec<EventCommand>>,
    #[cfg(unix)]
    interrupt: i32,
    #[cfg(windows)]
    port: usize,
}

fn event_handler() -> &'static EventHandler {
    static M: std::sync::OnceLock<EventHandler> = std::sync::OnceLock::new();
    M.get_or_init(start_handler)
}

#[cfg(unix)]
impl EventHandler {
    fn enviar(&self, c: EventCommand) {
        self.requests.lock().unwrap_or_else(|e| e.into_inner()).push(c);
        self.wake();
    }

    fn wake(&self) {
        unsafe extern "C" {
            fn write(fd: i32, b: *const std::ffi::c_void, n: usize) -> isize;
        }
        let b = [1u8];
        // SAFETY: o pipe vive o processo inteiro; um byte basta para acordar
        // (o pipe cheio já acorda o manipulador, então a falha é inócua).
        unsafe { write(self.interrupt, b.as_ptr().cast(), 1) };
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn pipe(fds: *mut i32) -> i32;
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn read(fd: i32, b: *mut std::ffi::c_void, n: usize) -> isize;
    fn shutdown(fd: i32, como: i32) -> i32;
}

#[cfg(unix)]
const F_GETFL: i32 = 3;
#[cfg(unix)]
const F_SETFL: i32 = 4;
#[cfg(unix)]
const F_SETFD: i32 = 2;
#[cfg(unix)]
const FD_CLOEXEC: i32 = 1;
#[cfg(all(unix, target_os = "linux"))]
const O_NONBLOCK: i32 = 0o4000;
#[cfg(all(unix, not(target_os = "linux")))]
const O_NONBLOCK: i32 = 0x4;

/// `FDUtils::SetNonBlocking`.
#[cfg(unix)]
fn set_nonblocking(fd: i32) -> bool {
    // SAFETY: consulta e muda as bandeiras de um descritor aberto.
    unsafe {
        let f = fcntl(fd, F_GETFL);
        f >= 0 && fcntl(fd, F_SETFL, f | O_NONBLOCK) >= 0
    }
}

/// `FDUtils::SetCloseOnExec`.
#[cfg(unix)]
fn close_on_exec(fd: i32) -> bool {
    // SAFETY: muda as bandeiras de um descritor aberto.
    unsafe { fcntl(fd, F_SETFD, FD_CLOEXEC) >= 0 }
}

#[cfg(unix)]
fn start_handler() -> EventHandler {
    let mut fds = [0i32; 2];
    // SAFETY: `fds` tem dois inteiros.
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        panic!("dart:io: falha ao criar o pipe do manipulador de eventos");
    }
    set_nonblocking(fds[0]);
    set_nonblocking(fds[1]);
    close_on_exec(fds[0]);
    close_on_exec(fds[1]);
    let read_side = fds[0];
    std::thread::Builder::new()
        .name("dart:io EventHandler".to_string())
        .spawn(move || EventLoop::new(read_side).run())
        .expect("dart:io: falha ao criar a thread do manipulador de eventos");
    EventHandler { requests: std::sync::Mutex::new(Vec::new()), interrupt: fds[1] }
}

/// O estado da thread do manipulador.
#[cfg(unix)]
struct EventLoop {
    poll: Poll,
    interrupt: i32,
    descriptors: crate::hash::HashMap<i64, DescriptorInfo>,
}

#[cfg(unix)]
impl EventLoop {
    fn new(interrupt: i32) -> EventLoop {
        let poll = Poll::new_area(interrupt);
        EventLoop { poll, interrupt, descriptors: crate::hash::HashMap::default() }
    }

    fn run(mut self) {
        let mut ready_list = Vec::new();
        loop {
            ready_list.clear();
            self.poll.wait_for(&mut ready_list);
            let mut interrupted = false;
            for &(fd, events) in &ready_list {
                if fd < 0 {
                    interrupted = true;
                    continue;
                }
                let Some(di) = self.descriptors.get_mut(&fd) else {
                    if io_trace() {
                        eprintln!("[io] evento fd={fd} eventos={events:#b} sem descritor");
                    }
                    continue;
                };
                let old_value = di.mask();
                if events & (1 << EVENT_ERROR) != 0 {
                    if io_trace() {
                        eprintln!("[io] evento fd={fd} eventos={events:#b} erro: todas as portas");
                    }
                    di.notify_all_waiters(events);
                    self.update(fd, old_value);
                } else if events != 0 {
                    let port = di.next_port();
                    if io_trace() {
                        eprintln!("[io] evento fd={fd} eventos={events:#b} porta={port} máscara {old_value:#b} -> {:#b}", di.mask());
                    }
                    self.update(fd, old_value);
                    post_event(port, events);
                }
            }
            if interrupted {
                self.handle_interrupt();
            }
        }
    }

    /// Reconcilia a sondagem com a máscara nova do descritor
    /// (`UpdateEpollInstance`).
    fn update(&mut self, fd: i64, old_value: i64) {
        let Some(di) = self.descriptors.get_mut(&fd) else { return };
        let new_area = di.mask();
        let listening = di.listening();
        if io_trace() && old_value != new_area {
            eprintln!("[io] sondagem fd={fd} de_escuta={listening} {old_value:#b} -> {new_area:#b}");
        }
        if old_value != 0 && new_area == 0 {
            self.poll.remove_it(fd);
        } else if old_value == 0 && new_area != 0 {
            if !self.poll.add(fd, new_area, listening) {
                di.notify_all_waiters(1 << EVENT_CLOSE);
            }
        } else if old_value != 0 && new_area != 0 && old_value != new_area {
            self.poll.remove_it(fd);
            if !self.poll.add(fd, new_area, listening) {
                di.notify_all_waiters(1 << EVENT_CLOSE);
            }
        }
    }

    /// `HandleInterruptFd`: esvazia o pipe e atende os pedidos.
    fn handle_interrupt(&mut self) {
        let mut garbage = [0u8; 256];
        // SAFETY: o pipe é não bloqueante; lê até esvaziar.
        while unsafe { read(self.interrupt, garbage.as_mut_ptr().cast(), garbage.len()) } > 0 {}
        let requests = std::mem::take(&mut *event_handler().requests.lock().unwrap_or_else(|e| e.into_inner()));
        for c in requests {
            self.handle_command(c);
        }
    }

    fn handle_command(&mut self, c: EventCommand) {
        // O pedido carrega uma referência do soquete (solta ao fim).
        // SAFETY: `c.sock` veio de `NativeSocket::new`, retido para o
        // pedido.
        let sock = unsafe { std::sync::Arc::from_raw(c.sock as *const NativeSocket) };
        let fd = sock.descriptor();
        if fd < 0 {
            return;
        }
        let listening = c.data & (1 << LISTENING_SOCKET) != 0;
        if io_trace() {
            eprintln!(
                "[io] pedido fd={fd} porta={} dados={:#x} de_escuta={listening} conhecido={}",
                c.port,
                c.data,
                self.descriptors.contains_key(&fd)
            );
        }
        let di = self.descriptors.entry(fd).or_insert_with(|| DescriptorInfo::new(listening));
        if is_command(c.data, COMMAND_SHUTDOWN_READ) {
            // SAFETY: descritor aberto deste soquete (SHUT_RD = 0).
            unsafe { shutdown(fd as i32, 0) };
        } else if is_command(c.data, COMMAND_SHUTDOWN_WRITE) {
            // SAFETY: descritor aberto deste soquete (SHUT_WR = 1).
            unsafe { shutdown(fd as i32, 1) };
        } else if is_command(c.data, COMMAND_CLOSE) {
            if c.data & (1 << SIGNAL_SOCKET) != 0 {
                clear_signal_by_descriptor(fd);
            }
            let old_value = di.mask();
            if c.port != 0 {
                di.remove_port(c.port);
            }
            self.update(fd, old_value);
            if listening {
                // Outros objetos podem dividir o descritor: só o último
                // fecha (`CloseSafe`).
                if listen_record().do_close(c.sock) {
                    self.descriptors.remove(&fd);
                    sock.close_descriptor();
                } else {
                    sock.release_descriptor();
                }
            } else {
                self.descriptors.remove(&fd);
                sock.close_descriptor();
            }
            post_event(c.port, 1 << EVENT_DESTROYED);
        } else if is_command(c.data, COMMAND_RETURN_TOKENS) {
            let old_value = di.mask();
            di.return_tokens(c.port, c.data & ((1 << COMMAND_CLOSE) - 1));
            self.update(fd, old_value);
        } else if is_command(c.data, COMMAND_MASK) {
            let old_value = di.mask();
            di.set_port_and_mask(c.port, c.data & EVENTS_MASK);
            self.update(fd, old_value);
        }
    }
}

// --- epoll (Linux) ---------------------------------------------------------

#[cfg(target_os = "linux")]
#[cfg_attr(target_arch = "x86_64", repr(C, packed))]
#[cfg_attr(not(target_arch = "x86_64"), repr(C))]
#[derive(Clone, Copy)]
struct EpollEvent {
    events: u32,
    data: u64,
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn epoll_create1(flags: i32) -> i32;
    fn epoll_ctl(ep: i32, op: i32, fd: i32, ev: *mut EpollEvent) -> i32;
    fn epoll_wait(ep: i32, evs: *mut EpollEvent, max: i32, wait_entry: i32) -> i32;
}

#[cfg(target_os = "linux")]
struct Poll {
    epoll: i32,
}

#[cfg(target_os = "linux")]
impl Poll {
    const EPOLLIN: u32 = 0x1;
    const EPOLLOUT: u32 = 0x4;
    const EPOLLERR: u32 = 0x8;
    const EPOLLHUP: u32 = 0x10;
    const EPOLLRDHUP: u32 = 0x2000;
    const EPOLLET: u32 = 1 << 31;

    fn new_area(interrupt: i32) -> Poll {
        // SAFETY: EPOLL_CLOEXEC = O_CLOEXEC.
        let epoll = unsafe { epoll_create1(0o2000000) };
        if epoll < 0 {
            panic!("dart:io: falha ao criar o epoll");
        }
        // O pipe de interrupção entra com o dado 0.
        let mut ev = EpollEvent { events: Self::EPOLLIN, data: 0 };
        // SAFETY: `ev` é válido durante a chamada.
        if unsafe { epoll_ctl(epoll, 1, interrupt, &mut ev) } != 0 {
            panic!("dart:io: falha ao vigiar o pipe de interrupção");
        }
        Poll { epoll }
    }

    /// `AddToEpollInstance`: `false` se o descritor não pôde entrar.
    fn add(&self, fd: i64, mask: i64, listening: bool) -> bool {
        let mut events = Self::EPOLLRDHUP;
        if mask & (1 << EVENT_IN) != 0 {
            events |= Self::EPOLLIN;
        }
        if mask & (1 << EVENT_OUT) != 0 {
            events |= Self::EPOLLOUT;
        }
        if !listening {
            events |= Self::EPOLLET;
        }
        let mut ev = EpollEvent { events, data: fd as u64 + 1 };
        // SAFETY: `ev` é válido durante a chamada (EPOLL_CTL_ADD = 1).
        unsafe { epoll_ctl(self.epoll, 1, fd as i32, &mut ev) == 0 }
    }

    fn remove_it(&self, fd: i64) {
        // SAFETY: EPOLL_CTL_DEL = 2; o evento é ignorado.
        unsafe { epoll_ctl(self.epoll, 2, fd as i32, std::ptr::null_mut()) };
    }

    /// Espera eventos: (descritor, máscara Dart); o descritor -1 é o pipe
    /// de interrupção.
    fn wait_for(&self, out: &mut Vec<(i64, i64)>) {
        let mut evs = [EpollEvent { events: 0, data: 0 }; 16];
        // SAFETY: o buffer tem 16 eventos.
        let n = unsafe { epoll_wait(self.epoll, evs.as_mut_ptr(), 16, -1) };
        for ev in evs.iter().take(n.max(0) as usize) {
            let (events, data) = (ev.events, ev.data);
            if data == 0 {
                out.push((-1, 0));
                continue;
            }
            out.push((data as i64 - 1, Self::dart_mask(events)));
        }
    }

    /// `GetPollEvents`.
    fn dart_mask(events: u32) -> i64 {
        if events & Self::EPOLLERR != 0 {
            return if events & Self::EPOLLIN != 0 { 1 << EVENT_ERROR } else { 0 };
        }
        let mut m = 0;
        if events & Self::EPOLLIN != 0 {
            m |= 1 << EVENT_IN;
        }
        if events & Self::EPOLLOUT != 0 {
            m |= 1 << EVENT_OUT;
        }
        if events & (Self::EPOLLHUP | Self::EPOLLRDHUP) != 0 {
            m |= 1 << EVENT_CLOSE;
        }
        m
    }
}

// --- kqueue (macOS e BSDs) -------------------------------------------------

#[cfg(all(unix, not(target_os = "linux")))]
#[repr(C)]
#[derive(Clone, Copy)]
struct EventoKqueue {
    ident: usize,
    filter_obj: i16,
    flag_bits: u16,
    fd_flags: u32,
    data: isize,
    udata: *mut std::ffi::c_void,
}

#[cfg(all(unix, not(target_os = "linux")))]
unsafe extern "C" {
    fn kqueue() -> i32;
    fn kevent(kq: i32, changes: *const EventoKqueue, nm: i32, events: *mut EventoKqueue, ne: i32, wait_entry: *const std::ffi::c_void) -> i32;
}

#[cfg(all(unix, not(target_os = "linux")))]
struct Poll {
    kq: i32,
    /// Os descritores que estão no kqueue e se são de escuta.
    watched: std::cell::RefCell<crate::hash::HashMap<i64, bool>>,
}

#[cfg(all(unix, not(target_os = "linux")))]
impl Poll {
    const EVFILT_READ: i16 = -1;
    const EVFILT_WRITE: i16 = -2;
    const EV_ADD: u16 = 0x1;
    const EV_DELETE: u16 = 0x2;
    const EV_CLEAR: u16 = 0x20;
    const EV_EOF: u16 = 0x8000;
    const EV_ERROR: u16 = 0x4000;

    fn event(fd: i64, filter_obj: i16, flag_bits: u16) -> EventoKqueue {
        EventoKqueue { ident: fd as usize, filter_obj, flag_bits, fd_flags: 0, data: 0, udata: (fd + 1) as usize as *mut std::ffi::c_void }
    }

    fn new_area(interrupt: i32) -> Poll {
        // SAFETY: cria o kqueue; o evento vive durante a chamada.
        let kq = unsafe { kqueue() };
        if kq < 0 {
            panic!("dart:io: falha ao criar o kqueue");
        }
        close_on_exec(kq);
        let mut ev = Self::event(i64::from(interrupt), Self::EVFILT_READ, Self::EV_ADD);
        ev.udata = std::ptr::null_mut();
        if unsafe { kevent(kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) } == -1 {
            panic!("dart:io: falha ao vigiar o pipe de interrupção");
        }
        Poll { kq, watched: std::cell::RefCell::new(crate::hash::HashMap::default()) }
    }

    /// `AddToKqueue`.
    fn add(&self, fd: i64, mask: i64, listening: bool) -> bool {
        let mut flag_bits = Self::EV_ADD;
        if !listening {
            flag_bits |= Self::EV_CLEAR;
        }
        let mut changes = Vec::with_capacity(2);
        if mask & (1 << EVENT_IN) != 0 {
            changes.push(Self::event(fd, Self::EVFILT_READ, flag_bits));
        }
        if mask & (1 << EVENT_OUT) != 0 {
            changes.push(Self::event(fd, Self::EVFILT_WRITE, flag_bits));
        }
        // SAFETY: as mudanças vivem durante a chamada.
        let ok = unsafe { kevent(self.kq, changes.as_ptr(), changes.len() as i32, std::ptr::null_mut(), 0, std::ptr::null()) } != -1;
        if io_trace() {
            eprintln!(
                "[io] kqueue adicionar fd={fd} máscara={mask:#b} de_escuta={listening} ok={ok} {}",
                if ok { String::new() } else { std::io::Error::last_os_error().to_string() }
            );
        }
        if ok {
            self.watched.borrow_mut().insert(fd, listening);
        }
        ok
    }

    /// `RemoveFromKqueue`.
    fn remove_it(&self, fd: i64) {
        let was_ready = self.watched.borrow_mut().remove(&fd).is_some();
        if io_trace() {
            eprintln!("[io] kqueue remover fd={fd} estava={was_ready}");
        }
        if !was_ready {
            return;
        }
        for filter_obj in [Self::EVFILT_READ, Self::EVFILT_WRITE] {
            let ev = Self::event(fd, filter_obj, Self::EV_DELETE);
            // SAFETY: a mudança vive durante a chamada; a ausência do filtro
            // é inócua.
            unsafe { kevent(self.kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
        }
    }

    fn wait_for(&self, out: &mut Vec<(i64, i64)>) {
        let empty = Self::event(0, 0, 0);
        let mut evs = [empty; 16];
        // SAFETY: o buffer tem 16 eventos; espera sem prazo.
        let n = unsafe { kevent(self.kq, std::ptr::null(), 0, evs.as_mut_ptr(), 16, std::ptr::null()) };
        for ev in evs.iter().take(n.max(0) as usize) {
            if ev.flag_bits & Self::EV_ERROR != 0 {
                continue;
            }
            if ev.udata.is_null() {
                out.push((-1, 0));
                continue;
            }
            let fd = ev.udata as usize as i64 - 1;
            let listening = self.watched.borrow().get(&fd).copied().unwrap_or(false);
            out.push((fd, Self::dart_mask(ev, listening)));
        }
    }

    /// `GetEvents` de `eventhandler_macos.cc`.
    fn dart_mask(ev: &EventoKqueue, listening: bool) -> i64 {
        let eof = ev.flag_bits & Self::EV_EOF != 0;
        if listening {
            let mut m = 0;
            if ev.filter_obj == Self::EVFILT_READ && eof {
                m = if ev.fd_flags != 0 { 1 << EVENT_ERROR } else { 1 << EVENT_CLOSE };
            }
            if m == 0 {
                m = 1 << EVENT_IN;
            }
            return m;
        }
        if ev.filter_obj == Self::EVFILT_READ {
            let mut m = 1 << EVENT_IN;
            if eof {
                if ev.fd_flags != 0 {
                    m = 1 << EVENT_ERROR;
                } else {
                    m |= 1 << EVENT_CLOSE;
                }
            }
            m
        } else {
            let mut m = 1 << EVENT_OUT;
            if eof && ev.fd_flags != 0 {
                m = 1 << EVENT_ERROR;
            }
            m
        }
    }
}

// ---------------------------------------------------------------------------
// O registro dos soquetes de escuta (`ListeningSocketRegistry`): o mesmo
// (endereço, porta) aberto por várias `ServerSocket` com `shared: true`
// divide um descritor.

/// Um soquete de escuta do sistema e quantos objetos Dart o usam.
struct ListeningSocket {
    address: Vec<u8>,
    port: i64,
    v6_only: bool,
    shared: bool,
    uses_list: usize,
    descriptor: i64,
    /// O caminho de um soquete de domínio Unix (apagado no último uso).
    unix_path: Option<Vec<u8>>,
}

#[derive(Default)]
struct ListenRecord {
    /// Os soquetes do sistema.
    sockets: Vec<ListeningSocket>,
    /// Soquete nativo (ponteiro) → descritor do sistema.
    per_socket: crate::hash::HashMap<i64, i64>,
}

fn listen_record() -> std::sync::MutexGuard<'static, ListenRecord> {
    static R: std::sync::OnceLock<std::sync::Mutex<ListenRecord>> = std::sync::OnceLock::new();
    R.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

impl ListenRecord {
    /// `CloseSafe`: solta um uso; `true` se o descritor do sistema deve ser
    /// fechado (era o último).
    fn do_close(&mut self, sock: i64) -> bool {
        let Some(fd) = self.per_socket.remove(&sock) else {
            return true;
        };
        let Some(i) = self.sockets.iter().position(|s| s.descriptor == fd) else {
            return true;
        };
        self.sockets[i].uses_list -= 1;
        if self.sockets[i].uses_list > 0 {
            return false;
        }
        let s = self.sockets.remove(i);
        #[cfg(unix)]
        if let Some(c) = s.unix_path {
            delete_unix_path(&c);
        }
        #[cfg(windows)]
        let _ = s;
        true
    }
}

// ---------------------------------------------------------------------------
// Os natives do `_EventHandler`.

/// `EventHandler_SendData(sender, sendPort, data)`: o comando `data` para o
/// soquete de `sender` (que passa a avisar em `sendPort`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_EventHandler_SendData(sender: i64, port: i64, data: i64) {
    let port = native_field(port);
    if sender == 0 {
        // Os timers da VM passam por aqui; os do nativo, não
        // (`eventos.rs`).
        return;
    }
    let p = native_field(sender);
    if p == 0 {
        throw_internal_error("No native peer");
        return;
    }
    // SAFETY: a referência do objeto está viva.
    unsafe { NativeSocket::de(p) }.port.store(port, std::sync::atomic::Ordering::Release);
    NativeSocket::retain_ref(p);
    event_handler().enviar(EventCommand { sock: p, port, data });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_EventHandler_TimerMillisecondClock() -> i64 {
    dartforge_nativo_Stopwatch_now() / 1_000_000
}
