// Native runtime: the VM's `dart:io` event handler (`runtime/bin/eventhandler*.cc`)
// and the socket object (`bin/socket.h`).
//
// A Dart `_NativeSocket` (TCP socket, process pipe, standard input) keeps in its
// native field a [`NativeSocket`]: the descriptor and the Dart port that receives
// its events, with reference counting. Dart asks the handler, through
// `EventHandler_SendData`, to watch the descriptor (a read/write mask), return
// tokens or close; the handler is a thread with epoll (Linux), kqueue (macOS) or an
// I/O completion port (Windows, in `io_windows_events.rs`) that posts to the
// socket's port the mask of the ready events (`kInEvent`, `kOutEvent`,
// `kCloseEvent`, `kErrorEvent`, `kDestroyedEvent`) — like the VM, including the
// token-based flow control (`TokenCounter`) and the listening sockets shared
// between ports (`DescriptorInfoMultiple`).
//
// Dart timers do not go through here: the isolate's event loop serves them
// (`events.rs`), and the `dart:async` overlay does not use `_EventHandler` for
// them.

/// `MessageFlags` of `eventhandler.h`.
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
// The native socket.

/// A descriptor that Dart uses as a `_NativeSocket` (the VM's `Socket`).
struct NativeSocket {
    descriptor: std::sync::atomic::AtomicI64,
    /// The Dart port of the events (the `_NativeSocket`'s `eventPort`).
    port: std::sync::atomic::AtomicI64,
}

impl NativeSocket {
    /// Creates the socket with one reference and returns its pointer.
    fn new(descriptor: i64) -> i64 {
        std::sync::Arc::into_raw(std::sync::Arc::new(NativeSocket {
            descriptor: std::sync::atomic::AtomicI64::new(descriptor),
            port: std::sync::atomic::AtomicI64::new(0),
        })) as i64
    }

    /// # Safety
    /// `p` came from [`NativeSocket::new`] and the reference still exists.
    unsafe fn of<'a>(p: i64) -> &'a NativeSocket {
        // SAFETY: guaranteed by the caller.
        unsafe { &*(p as *const NativeSocket) }
    }

    fn retain_ref(p: i64) {
        // SAFETY: `p` is a live pointer from `new`.
        unsafe { std::sync::Arc::increment_strong_count(p as *const NativeSocket) };
    }

    fn free(p: i64) {
        // SAFETY: `p` is a live pointer from `new`, with this reference.
        unsafe { std::sync::Arc::decrement_strong_count(p as *const NativeSocket) };
    }

    fn descriptor(&self) -> i64 {
        self.descriptor.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Closes the system resource (the descriptor, on Unix; the `Handle`'s
    /// reference, on Windows, whose closing the handler has already done).
    fn close_descriptor(&self) {
        let d = self.descriptor.swap(-1, std::sync::atomic::Ordering::AcqRel);
        if d >= 0 {
            close_socket_descriptor(d);
        }
    }

    /// `Socket::CloseFd`: the object leaves the descriptor without closing it on
    /// Unix (the handler closes it, or nobody does: standard input); on Windows it
    /// releases the `Handle`'s reference.
    fn release_descriptor(&self) {
        #[cfg(unix)]
        self.descriptor.store(-1, std::sync::atomic::Ordering::Release);
        #[cfg(windows)]
        self.close_descriptor();
    }
}

/// On Windows each object owns one reference of the `Handle` (the `descriptor`);
/// it is released with the object, if it was not already.
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
    // SAFETY: `d` is a descriptor of this socket, closed once.
    unsafe { close(d as i32) };
}

/// Another reference to the descriptor of a shared listening socket (on Unix the
/// descriptor is the same, without counting).
#[cfg(unix)]
fn retain_descriptor(d: i64) -> i64 {
    d
}

/// The standard input socket (`SocketBase::GetStdioHandle`): the descriptor itself
/// on Unix.
#[cfg(unix)]
fn default_handle(num: i64) -> i64 {
    default_descriptor(num)
}

/// The finalizers of a `_NativeSocket` collected without `close` (the
/// `*SocketFinalizer`s of `socket.cc`).
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
    // SAFETY: the object's reference still exists; it is released here.
    unsafe { NativeSocket::of(pair as i64) }.release_descriptor();
    NativeSocket::free(pair as i64);
}

/// Sends the close command of a collected socket to the handler: the object's
/// reference passes to the command, which releases it.
fn close_collected_socket(p: i64, data: i64) {
    // SAFETY: the collected object's reference still exists.
    let port = unsafe { NativeSocket::of(p) }.port.load(std::sync::atomic::Ordering::Acquire);
    event_handler().send(EventCommand { sock: p, port, data });
}

/// `Socket::SetSocketIdNativeField`: creates the socket of `descriptor` in the
/// native field of `object`, with the finalizer.
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

/// `Socket::GetSocketIdNativeField`: the object's socket, or throws the internal
/// error ("No native peer").
fn object_socket<'a>(object: i64) -> Option<&'a NativeSocket> {
    let p = native_field(object);
    if p == 0 {
        throw_internal_error("No native peer");
        return None;
    }
    // SAFETY: the object's reference lives until it is collected.
    Some(unsafe { NativeSocket::of(p) })
}

// ---------------------------------------------------------------------------
// The tokens and the interested parties of a descriptor (`DescriptorInfo*Mixin`).

/// What the handler knows about a descriptor.
enum DescriptorInfo {
    /// An ordinary socket: one port, one mask and 16 tokens.
    Single { port: i64, mask: i64, tokens: i64 },
    /// A shared listening socket: each port with 4 tokens and whether it wants to
    /// read; the ready ones take turns (`active_readers_`).
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

    /// The port to notify of an I/O event (and one token less).
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

    /// Notifies every port (close, error, destruction).
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

    /// The mask to watch now (empty without tokens).
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

/// `DARTFORGE_IO_RASTRO=1`: one line on stderr per poll event, per request to the
/// handler and per change of what the poll watches (the diagnosis of `dart:io`
/// defects that only show up on one system).
#[cfg(unix)]
fn io_trace() -> bool {
    static R: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *R.get_or_init(|| std::env::var("DARTFORGE_IO_RASTRO").is_ok_and(|v| v == "1"))
}

/// `DartUtils::PostInt32`: the event mask on the port.
fn post_event(port: i64, events: i64) {
    if port != 0 {
        post(port, Graph::from_int(events));
    }
}

// ---------------------------------------------------------------------------
// The handler.

/// A request to the handler (the VM's `InterruptMessage`): the socket (with the
/// reference the request carries), the port and the data.
struct EventCommand {
    sock: i64,
    port: i64,
    data: i64,
}

/// The handler thread and its request queue; one byte in the interrupt pipe wakes
/// it. On Windows the request goes through the completion port
/// (`io_windows_events.rs`).
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
    fn send(&self, c: EventCommand) {
        self.requests.lock().unwrap_or_else(|e| e.into_inner()).push(c);
        self.wake();
    }

    fn wake(&self) {
        unsafe extern "C" {
            fn write(fd: i32, b: *const std::ffi::c_void, n: usize) -> isize;
        }
        let b = [1u8];
        // SAFETY: the pipe lives for the whole process; one byte is enough to wake
        // it (a full pipe already wakes the handler, so the failure is harmless).
        unsafe { write(self.interrupt, b.as_ptr().cast(), 1) };
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn pipe(fds: *mut i32) -> i32;
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn read(fd: i32, b: *mut std::ffi::c_void, n: usize) -> isize;
    fn shutdown(fd: i32, how: i32) -> i32;
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
    // SAFETY: queries and changes the flags of an open descriptor.
    unsafe {
        let f = fcntl(fd, F_GETFL);
        f >= 0 && fcntl(fd, F_SETFL, f | O_NONBLOCK) >= 0
    }
}

/// `FDUtils::SetCloseOnExec`.
#[cfg(unix)]
fn close_on_exec(fd: i32) -> bool {
    // SAFETY: changes the flags of an open descriptor.
    unsafe { fcntl(fd, F_SETFD, FD_CLOEXEC) >= 0 }
}

#[cfg(unix)]
fn start_handler() -> EventHandler {
    let mut fds = [0i32; 2];
    // SAFETY: `fds` has two integers.
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        panic!("dart:io: failed to create the event handler pipe");
    }
    set_nonblocking(fds[0]);
    set_nonblocking(fds[1]);
    close_on_exec(fds[0]);
    close_on_exec(fds[1]);
    let read_side = fds[0];
    std::thread::Builder::new()
        .name("dart:io EventHandler".to_string())
        .spawn(move || EventLoop::new(read_side).run())
        .expect("dart:io: failed to create the event handler thread");
    EventHandler { requests: std::sync::Mutex::new(Vec::new()), interrupt: fds[1] }
}

/// The state of the handler thread.
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
                        eprintln!("[io] event fd={fd} events={events:#b} without descriptor");
                    }
                    continue;
                };
                let old_value = di.mask();
                if events & (1 << EVENT_ERROR) != 0 {
                    if io_trace() {
                        eprintln!("[io] event fd={fd} events={events:#b} error: all ports");
                    }
                    di.notify_all_waiters(events);
                    self.update(fd, old_value);
                } else if events != 0 {
                    let port = di.next_port();
                    if io_trace() {
                        eprintln!("[io] event fd={fd} events={events:#b} port={port} mask {old_value:#b} -> {:#b}", di.mask());
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

    /// Reconciles the poll with the descriptor's new mask (`UpdateEpollInstance`).
    fn update(&mut self, fd: i64, old_value: i64) {
        let Some(di) = self.descriptors.get_mut(&fd) else { return };
        let new_area = di.mask();
        let listening = di.listening();
        if io_trace() && old_value != new_area {
            eprintln!("[io] poll fd={fd} listening={listening} {old_value:#b} -> {new_area:#b}");
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

    /// `HandleInterruptFd`: drains the pipe and serves the requests.
    fn handle_interrupt(&mut self) {
        let mut garbage = [0u8; 256];
        // SAFETY: the pipe is non-blocking; reads until it is empty.
        while unsafe { read(self.interrupt, garbage.as_mut_ptr().cast(), garbage.len()) } > 0 {}
        let requests = std::mem::take(&mut *event_handler().requests.lock().unwrap_or_else(|e| e.into_inner()));
        for c in requests {
            self.handle_command(c);
        }
    }

    fn handle_command(&mut self, c: EventCommand) {
        // The request carries a reference to the socket (released at the end).
        // SAFETY: `c.sock` came from `NativeSocket::new`, retained for the
        // request.
        let sock = unsafe { std::sync::Arc::from_raw(c.sock as *const NativeSocket) };
        let fd = sock.descriptor();
        if fd < 0 {
            return;
        }
        let listening = c.data & (1 << LISTENING_SOCKET) != 0;
        if io_trace() {
            eprintln!(
                "[io] request fd={fd} port={} data={:#x} listening={listening} known={}",
                c.port,
                c.data,
                self.descriptors.contains_key(&fd)
            );
        }
        let di = self.descriptors.entry(fd).or_insert_with(|| DescriptorInfo::new(listening));
        if is_command(c.data, COMMAND_SHUTDOWN_READ) {
            // SAFETY: an open descriptor of this socket (SHUT_RD = 0).
            unsafe { shutdown(fd as i32, 0) };
        } else if is_command(c.data, COMMAND_SHUTDOWN_WRITE) {
            // SAFETY: an open descriptor of this socket (SHUT_WR = 1).
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
                // Other objects may share the descriptor: only the last one
                // closes it (`CloseSafe`).
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
            panic!("dart:io: failed to create the epoll");
        }
        // The interrupt pipe goes in with data 0.
        let mut ev = EpollEvent { events: Self::EPOLLIN, data: 0 };
        // SAFETY: `ev` is valid during the call.
        if unsafe { epoll_ctl(epoll, 1, interrupt, &mut ev) } != 0 {
            panic!("dart:io: failed to watch the interrupt pipe");
        }
        Poll { epoll }
    }

    /// `AddToEpollInstance`: `false` if the descriptor could not be added.
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
        // SAFETY: `ev` is valid during the call (EPOLL_CTL_ADD = 1).
        unsafe { epoll_ctl(self.epoll, 1, fd as i32, &mut ev) == 0 }
    }

    fn remove_it(&self, fd: i64) {
        // SAFETY: EPOLL_CTL_DEL = 2; the event is ignored.
        unsafe { epoll_ctl(self.epoll, 2, fd as i32, std::ptr::null_mut()) };
    }

    /// Waits for events: (descriptor, Dart mask); descriptor -1 is the interrupt
    /// pipe.
    fn wait_for(&self, out: &mut Vec<(i64, i64)>) {
        let mut evs = [EpollEvent { events: 0, data: 0 }; 16];
        // SAFETY: the buffer has 16 events.
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

// --- kqueue (macOS and the BSDs) -------------------------------------------

#[cfg(all(unix, not(target_os = "linux")))]
#[repr(C)]
#[derive(Clone, Copy)]
struct KqueueEvent {
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
    fn kevent(kq: i32, changes: *const KqueueEvent, nm: i32, events: *mut KqueueEvent, ne: i32, wait_entry: *const std::ffi::c_void) -> i32;
}

#[cfg(all(unix, not(target_os = "linux")))]
struct Poll {
    kq: i32,
    /// The descriptors that are in the kqueue and whether they are listening.
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

    fn event(fd: i64, filter_obj: i16, flag_bits: u16) -> KqueueEvent {
        KqueueEvent { ident: fd as usize, filter_obj, flag_bits, fd_flags: 0, data: 0, udata: (fd + 1) as usize as *mut std::ffi::c_void }
    }

    fn new_area(interrupt: i32) -> Poll {
        // SAFETY: creates the kqueue; the event lives during the call.
        let kq = unsafe { kqueue() };
        if kq < 0 {
            panic!("dart:io: failed to create the kqueue");
        }
        close_on_exec(kq);
        let mut ev = Self::event(i64::from(interrupt), Self::EVFILT_READ, Self::EV_ADD);
        ev.udata = std::ptr::null_mut();
        if unsafe { kevent(kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) } == -1 {
            panic!("dart:io: failed to watch the interrupt pipe");
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
        // SAFETY: the changes live during the call.
        let ok = unsafe { kevent(self.kq, changes.as_ptr(), changes.len() as i32, std::ptr::null_mut(), 0, std::ptr::null()) } != -1;
        if io_trace() {
            eprintln!(
                "[io] kqueue add fd={fd} mask={mask:#b} listening={listening} ok={ok} {}",
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
            eprintln!("[io] kqueue remove fd={fd} was_ready={was_ready}");
        }
        if !was_ready {
            return;
        }
        for filter_obj in [Self::EVFILT_READ, Self::EVFILT_WRITE] {
            let ev = Self::event(fd, filter_obj, Self::EV_DELETE);
            // SAFETY: the change lives during the call; the absence of the filter
            // is harmless.
            unsafe { kevent(self.kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
        }
    }

    fn wait_for(&self, out: &mut Vec<(i64, i64)>) {
        let empty = Self::event(0, 0, 0);
        let mut evs = [empty; 16];
        // SAFETY: the buffer has 16 events; waits without a deadline.
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

    /// `GetEvents` of `eventhandler_macos.cc`.
    fn dart_mask(ev: &KqueueEvent, listening: bool) -> i64 {
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
// The registry of listening sockets (`ListeningSocketRegistry`): the same
// (address, port) opened by several `ServerSocket`s with `shared: true` shares a
// descriptor.

/// A system listening socket and how many Dart objects use it.
struct ListeningSocket {
    address: Vec<u8>,
    port: i64,
    v6_only: bool,
    shared: bool,
    uses_list: usize,
    descriptor: i64,
    /// The path of a Unix domain socket (deleted on the last use).
    unix_path: Option<Vec<u8>>,
}

#[derive(Default)]
struct ListenRecord {
    /// The system sockets.
    sockets: Vec<ListeningSocket>,
    /// Native socket (pointer) → system descriptor.
    per_socket: crate::hash::HashMap<i64, i64>,
}

fn listen_record() -> std::sync::MutexGuard<'static, ListenRecord> {
    static R: std::sync::OnceLock<std::sync::Mutex<ListenRecord>> = std::sync::OnceLock::new();
    R.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

impl ListenRecord {
    /// `CloseSafe`: releases one use; `true` if the system descriptor must be
    /// closed (it was the last one).
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
// The `_EventHandler` natives.

/// `EventHandler_SendData(sender, sendPort, data)`: command `data` for the socket
/// of `sender` (which starts notifying on `sendPort`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_EventHandler_SendData(sender: i64, port: i64, data: i64) {
    let port = native_field(port);
    if sender == 0 {
        // The VM's timers go through here; the native ones do not (`events.rs`).
        return;
    }
    let p = native_field(sender);
    if p == 0 {
        throw_internal_error("No native peer");
        return;
    }
    // SAFETY: the object's reference is alive.
    unsafe { NativeSocket::of(p) }.port.store(port, std::sync::atomic::Ordering::Release);
    NativeSocket::retain_ref(p);
    event_handler().send(EventCommand { sock: p, port, data });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_EventHandler_TimerMillisecondClock() -> i64 {
    dartforge_native_Stopwatch_now() / 1_000_000
}
