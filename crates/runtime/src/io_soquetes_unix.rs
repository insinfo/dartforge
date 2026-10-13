// Runtime nativo: a camada Unix (Linux e macOS) dos soquetes do `dart:io`
// (`socket_base_posix.cc`, `socket_linux.cc`, `socket_macos.cc` da VM): as
// constantes e estruturas do sistema, os endereços e as operações que os
// natives de `io_soquetes.rs` chamam. A camada do Windows, com os mesmos
// nomes, está em `io_windows_soquetes.rs`.
// ---------------------------------------------------------------------------
// Constantes do sistema.

#[cfg(target_os = "linux")]
mod network_only {
    pub const AF_UNIX: u16 = 1;
    pub const AF_INET: u16 = 2;
    pub const AF_INET6: u16 = 10;
    pub const SOCK_STREAM: i32 = 1;
    pub const SOCK_DGRAM: i32 = 2;
    pub const SOL_SOCKET: i32 = 1;
    pub const SO_REUSEADDR: i32 = 2;
    pub const SO_ERROR: i32 = 4;
    pub const SO_BROADCAST: i32 = 6;
    pub const SO_REUSEPORT: i32 = 15;
    pub const IPPROTO_IP: i32 = 0;
    pub const IPPROTO_TCP: i32 = 6;
    pub const IPPROTO_UDP: i32 = 17;
    pub const IPPROTO_IPV6: i32 = 41;
    pub const IPV6_V6ONLY: i32 = 26;
    pub const TCP_NODELAY: i32 = 1;
    pub const IP_MULTICAST_IF: i32 = 32;
    pub const IP_MULTICAST_TTL: i32 = 33;
    pub const IP_MULTICAST_LOOP: i32 = 34;
    pub const IPV6_MULTICAST_IF: i32 = 17;
    pub const IPV6_MULTICAST_HOPS: i32 = 18;
    pub const IPV6_MULTICAST_LOOP: i32 = 19;
    pub const FIONREAD: std::ffi::c_ulong = 0x541B;
    pub const EINPROGRESS: i32 = 115;
    pub const EAGAIN: i32 = 11;
    pub const EADDRINUSE: i32 = 98;
    pub const EADDRNOTAVAIL: i32 = 99;
    pub const EINVAL: i32 = 22;
    pub const SOMAXCONN: i32 = 4096;
    pub const MSG_PEEK: i32 = 2;
    pub const AI_ADDRCONFIG: i32 = 0x20;
    pub const NI_NUMERICHOST: i32 = 1;
    pub const NI_NAMEREQD: i32 = 8;
    pub const UNIX_PATH_SIZE: usize = 108;
    pub const SCM_RIGHTS: i32 = 1;
    pub const MSG_NOSIGNAL: i32 = 0x4000;
    pub const MSG_CMSG_CLOEXEC: i32 = 0x4000_0000;
    /// Erros de `accept` que não fecham o servidor (`IsTemporaryAcceptError`).
    pub const TRANSIENT_ACCEPT_ERRORS: &[i32] = &[11, 100, 71, 92, 112, 64, 113, 95, 101];
}

#[cfg(all(unix, not(target_os = "linux")))]
mod network_only {
    pub const AF_UNIX: u16 = 1;
    pub const AF_INET: u16 = 2;
    pub const AF_INET6: u16 = 30;
    pub const SOCK_STREAM: i32 = 1;
    pub const SOCK_DGRAM: i32 = 2;
    pub const SOL_SOCKET: i32 = 0xffff;
    pub const SO_REUSEADDR: i32 = 0x4;
    pub const SO_ERROR: i32 = 0x1007;
    pub const SO_BROADCAST: i32 = 0x20;
    pub const SO_REUSEPORT: i32 = 0x200;
    pub const SO_NOSIGPIPE: i32 = 0x1022;
    pub const IPPROTO_IP: i32 = 0;
    pub const IPPROTO_TCP: i32 = 6;
    pub const IPPROTO_UDP: i32 = 17;
    pub const IPPROTO_IPV6: i32 = 41;
    pub const IPV6_V6ONLY: i32 = 27;
    pub const TCP_NODELAY: i32 = 1;
    pub const IP_MULTICAST_IF: i32 = 9;
    pub const IP_MULTICAST_TTL: i32 = 10;
    pub const IP_MULTICAST_LOOP: i32 = 11;
    pub const IPV6_MULTICAST_IF: i32 = 9;
    pub const IPV6_MULTICAST_HOPS: i32 = 10;
    pub const IPV6_MULTICAST_LOOP: i32 = 11;
    pub const FIONREAD: std::ffi::c_ulong = 0x4004_667f;
    pub const EINPROGRESS: i32 = 36;
    pub const EAGAIN: i32 = 35;
    pub const EADDRINUSE: i32 = 48;
    pub const EADDRNOTAVAIL: i32 = 49;
    pub const EINVAL: i32 = 22;
    pub const SOMAXCONN: i32 = 128;
    pub const MSG_PEEK: i32 = 2;
    pub const AI_ADDRCONFIG: i32 = 0x400;
    pub const NI_NUMERICHOST: i32 = 2;
    pub const NI_NAMEREQD: i32 = 4;
    pub const UNIX_PATH_SIZE: usize = 104;
    pub const SCM_RIGHTS: i32 = 1;
    /// O macOS não tem `MSG_NOSIGNAL` (os soquetes já têm `SO_NOSIGPIPE`)
    /// nem `MSG_CMSG_CLOEXEC`.
    pub const MSG_NOSIGNAL: i32 = 0;
    pub const MSG_CMSG_CLOEXEC: i32 = 0;
    pub const TRANSIENT_ACCEPT_ERRORS: &[i32] = &[35, 50, 100, 42, 64, 65, 102, 51];
}

#[cfg(unix)]
unsafe extern "C" {
    fn socket(domain: i32, tipo: i32, protocol: i32) -> i32;
    fn connect(fd: i32, e: *const u8, n: u32) -> i32;
    fn bind(fd: i32, e: *const u8, n: u32) -> i32;
    fn listen(fd: i32, queue: i32) -> i32;
    fn accept(fd: i32, e: *mut u8, n: *mut u32) -> i32;
    fn getsockname(fd: i32, e: *mut u8, n: *mut u32) -> i32;
    fn getpeername(fd: i32, e: *mut u8, n: *mut u32) -> i32;
    fn setsockopt(fd: i32, level: i32, option: i32, v: *const std::ffi::c_void, n: u32) -> i32;
    fn getsockopt(fd: i32, level: i32, option: i32, v: *mut std::ffi::c_void, n: *mut u32) -> i32;
    fn recvfrom(fd: i32, b: *mut u8, n: usize, flags: i32, e: *mut u8, en: *mut u32) -> isize;
    fn sendto(fd: i32, b: *const u8, n: usize, flags: i32, e: *const u8, en: u32) -> isize;
    fn write(fd: i32, b: *const std::ffi::c_void, n: usize) -> isize;
    fn getnameinfo(e: *const u8, en: u32, host: *mut std::ffi::c_char, hn: u32, serv: *mut std::ffi::c_char, sn: u32, flags: i32) -> i32;
    fn getaddrinfo(host: *const std::ffi::c_char, serv: *const std::ffi::c_char, hints: *const EnderecoInfo, res: *mut *mut EnderecoInfo) -> i32;
    fn freeaddrinfo(res: *mut EnderecoInfo);
    fn gai_strerror(code: i32) -> *const std::ffi::c_char;
    fn inet_pton(family: i32, text: *const std::ffi::c_char, dest: *mut u8) -> i32;
    fn getifaddrs(res: *mut *mut InterfaceAddress) -> i32;
    fn freeifaddrs(res: *mut InterfaceAddress);
    fn if_nametoindex(name: *const std::ffi::c_char) -> u32;
    fn unlink(file_path: *const std::ffi::c_char) -> i32;
    fn sendmsg(fd: i32, m: *const CabecalhoDeMensagem, flags: i32) -> isize;
    fn recvmsg(fd: i32, m: *mut CabecalhoDeMensagem, flags: i32) -> isize;
}

/// `struct iovec`.
#[cfg(unix)]
#[repr(C)]
struct Slice {
    base: *mut u8,
    size: usize,
}

/// `struct msghdr` (no macOS, `msg_iovlen` é `int` e `msg_controllen` é
/// `socklen_t`).
#[cfg(unix)]
#[repr(C)]
struct CabecalhoDeMensagem {
    name: *mut u8,
    name_size: u32,
    slices: *mut Slice,
    #[cfg(target_os = "linux")]
    n_slices: usize,
    #[cfg(not(target_os = "linux"))]
    n_slices: i32,
    control: *mut u8,
    #[cfg(target_os = "linux")]
    control_size: usize,
    #[cfg(not(target_os = "linux"))]
    control_size: u32,
    flag_bits: i32,
}

/// O `struct cmsghdr` e o `CMSG_ALIGN`: no Linux, `cmsg_len` é `size_t` e o
/// alinhamento é o do `size_t`; no macOS, `socklen_t` e 4 bytes.
#[cfg(target_os = "linux")]
mod cmsg {
    pub const TAMANHO_DO_CABECALHO: usize = 16;
    pub const ALIGNMENT: usize = 8;
    pub fn write_size(b: &mut [u8], n: usize) {
        b[..8].copy_from_slice(&(n as u64).to_ne_bytes());
    }
    pub fn read_size(b: &[u8]) -> usize {
        u64::from_ne_bytes(b[..8].try_into().unwrap()) as usize
    }
    pub const LEVEL_SHIFT: usize = 8;
}

#[cfg(all(unix, not(target_os = "linux")))]
mod cmsg {
    pub const TAMANHO_DO_CABECALHO: usize = 12;
    pub const ALIGNMENT: usize = 4;
    pub fn write_size(b: &mut [u8], n: usize) {
        b[..4].copy_from_slice(&(n as u32).to_ne_bytes());
    }
    pub fn read_size(b: &[u8]) -> usize {
        u32::from_ne_bytes(b[..4].try_into().unwrap()) as usize
    }
    pub const LEVEL_SHIFT: usize = 4;
}

/// `CMSG_ALIGN`.
#[cfg(unix)]
fn align_cmsg(n: usize) -> usize {
    (n + cmsg::ALIGNMENT - 1) & !(cmsg::ALIGNMENT - 1)
}

/// `struct addrinfo` (a ordem de `ai_addr` e `ai_canonname` difere).
#[cfg(unix)]
#[repr(C)]
struct EnderecoInfo {
    ai_flags: i32,
    ai_family: i32,
    ai_socktype: i32,
    ai_protocol: i32,
    ai_addrlen: u32,
    #[cfg(target_os = "linux")]
    ai_addr: *const u8,
    ai_canonname: *const std::ffi::c_char,
    #[cfg(not(target_os = "linux"))]
    ai_addr: *const u8,
    ai_next: *mut EnderecoInfo,
}

/// `struct ifaddrs`.
#[cfg(unix)]
#[repr(C)]
struct InterfaceAddress {
    ifa_next: *mut InterfaceAddress,
    ifa_name: *const std::ffi::c_char,
    ifa_flags: u32,
    ifa_addr: *const u8,
    ifa_netmask: *const u8,
    ifa_ifu: *const u8,
    ifa_data: *const std::ffi::c_void,
}

// ---------------------------------------------------------------------------
// Endereços (`RawAddr`/`SocketAddress`).

#[cfg(unix)]
impl OsAddress {
    fn empty() -> OsAddress {
        OsAddress { bytes: [0; 128], size: 128 }
    }

    /// A família (`sa_family`; no macOS vem depois do `sa_len`).
    fn family(&self) -> u16 {
        if cfg!(target_os = "linux") {
            u16::from_ne_bytes([self.bytes[0], self.bytes[1]])
        } else {
            u16::from(self.bytes[1])
        }
    }

    fn write_family(&mut self, f: u16, size: u32) {
        if cfg!(target_os = "linux") {
            self.bytes[..2].copy_from_slice(&f.to_ne_bytes());
        } else {
            self.bytes[0] = size as u8;
            self.bytes[1] = f as u8;
        }
        self.size = size;
    }

    /// `SocketAddress::GetSockAddr`: 4 bytes → IPv4, 16 → IPv6.
    fn from_ip(ip: &[u8]) -> Option<OsAddress> {
        let mut e = OsAddress::empty();
        match ip.len() {
            4 => {
                e.write_family(network_only::AF_INET, 16);
                e.bytes[4..8].copy_from_slice(ip);
            }
            16 => {
                e.write_family(network_only::AF_INET6, 28);
                e.bytes[8..24].copy_from_slice(ip);
            }
            _ => return None,
        }
        Some(e)
    }

    /// Um endereço de domínio Unix (`sockaddr_un`).
    fn from_unix_path(file_path: &[u8]) -> IoResult<OsAddress> {
        if file_path.len() >= network_only::UNIX_PATH_SIZE {
            return Err(OsError::from_code(network_only::EINVAL));
        }
        let mut e = OsAddress::empty();
        e.write_family(network_only::AF_UNIX, (2 + file_path.len() + 1) as u32);
        e.bytes[2..2 + file_path.len()].copy_from_slice(file_path);
        Ok(e)
    }

    fn from_pointer(p: *const u8, n: u32) -> OsAddress {
        let mut e = OsAddress::empty();
        let n = (n as usize).min(128);
        // SAFETY: `p` aponta para um `sockaddr` de `n` bytes do sistema.
        e.bytes[..n].copy_from_slice(unsafe { std::slice::from_raw_parts(p, n) });
        e.size = n as u32;
        e
    }

    fn is_ipv6(&self) -> bool {
        self.family() == network_only::AF_INET6
    }

    /// `SocketAddress::GetType`.
    fn tipo(&self) -> i64 {
        match self.family() {
            network_only::AF_INET => TYPE_IPV4,
            network_only::AF_INET6 => TYPE_IPV6,
            _ => TYPE_UNIX,
        }
    }

    fn port(&self) -> i64 {
        match self.family() {
            network_only::AF_INET | network_only::AF_INET6 => i64::from(u16::from_be_bytes([self.bytes[2], self.bytes[3]])),
            _ => 0,
        }
    }

    fn write_port(&mut self, p: i64) {
        self.bytes[2..4].copy_from_slice(&(p as u16).to_be_bytes());
    }

    fn scope(&self) -> i64 {
        if self.is_ipv6() { i64::from(u32::from_ne_bytes(self.bytes[24..28].try_into().expect("4 bytes"))) } else { 0 }
    }

    fn write_scope(&mut self, s: i64) {
        if self.is_ipv6() {
            self.bytes[24..28].copy_from_slice(&(s as u32).to_ne_bytes());
        }
    }

    /// Os bytes do endereço IP (`SocketAddress::ToTypedData`).
    fn ip(&self) -> Vec<u8> {
        if self.is_ipv6() { self.bytes[8..24].to_vec() } else { self.bytes[4..8].to_vec() }
    }

    /// O tamanho do `sockaddr` da família (`GetAddrLength`).
    fn family_size(&self) -> u32 {
        match self.family() {
            network_only::AF_INET => 16,
            network_only::AF_INET6 => 28,
            _ => self.size,
        }
    }

    /// O texto numérico (`getnameinfo(NI_NUMERICHOST)`, com `%interface` no
    /// IPv6 de escopo local); o caminho, no domínio Unix.
    fn text(&self) -> String {
        if self.family() == network_only::AF_UNIX {
            let file_path = &self.bytes[2..self.size.max(2) as usize];
            let end = file_path.iter().position(|&c| c == 0).unwrap_or(file_path.len());
            return String::from_utf8_lossy(&file_path[..end]).into_owned();
        }
        let mut b = [0 as std::ffi::c_char; 64];
        // SAFETY: o buffer tem 64 bytes (INET6_ADDRSTRLEN com escopo).
        let r = unsafe {
            getnameinfo(self.bytes.as_ptr(), self.family_size(), b.as_mut_ptr(), b.len() as u32, std::ptr::null_mut(), 0, network_only::NI_NUMERICHOST)
        };
        if r != 0 {
            return String::new();
        }
        // SAFETY: `getnameinfo` termina o texto em NUL.
        unsafe { std::ffi::CStr::from_ptr(b.as_ptr()) }.to_string_lossy().into_owned()
    }
}

// ---------------------------------------------------------------------------
// Operações do sistema (Unix).

#[cfg(unix)]
fn last_error() -> OsError {
    OsError::de(&std::io::Error::last_os_error())
}

/// Fecha `fd` preservando o erro corrente (`FDUtils::SaveErrorAndClose`).
#[cfg(unix)]
fn close_with_error(fd: i32) -> OsError {
    let e = last_error();
    close_socket_descriptor(i64::from(fd));
    e
}

/// Um soquete não bloqueante e fechado no `exec`.
#[cfg(unix)]
fn create_socket(family: u16, tipo: i32, protocol: i32) -> IoResult<i32> {
    #[cfg(target_os = "linux")]
    let fd = {
        const SOCK_NONBLOCK: i32 = 0o4000;
        const SOCK_CLOEXEC: i32 = 0o2000000;
        // SAFETY: cria um soquete.
        unsafe { socket(i32::from(family), tipo | SOCK_NONBLOCK | SOCK_CLOEXEC, protocol) }
    };
    #[cfg(not(target_os = "linux"))]
    let fd = {
        // SAFETY: cria um soquete.
        let fd = unsafe { socket(i32::from(family), tipo, protocol) };
        if fd >= 0 && !(close_on_exec(fd) && set_nonblocking(fd)) {
            return Err(close_with_error(fd));
        }
        if fd >= 0 {
            // Sem SIGPIPE ao escrever num soquete fechado (a VM do macOS faz
            // o mesmo).
            set_int_option(fd, network_only::SOL_SOCKET, network_only::SO_NOSIGPIPE, 1);
        }
        fd
    };
    if fd < 0 {
        return Err(last_error());
    }
    Ok(fd)
}

#[cfg(unix)]
fn set_int_option(fd: i32, level: i32, option: i32, value: i32) -> bool {
    // SAFETY: `value` vive durante a chamada.
    unsafe { setsockopt(fd, level, option, (&value as *const i32).cast(), 4) == 0 }
}

#[cfg(unix)]
fn get_int_option(fd: i32, level: i32, option: i32) -> Option<i32> {
    let mut v = 0i32;
    let mut n = 4u32;
    // SAFETY: `v` tem 4 bytes.
    (unsafe { getsockopt(fd, level, option, (&mut v as *mut i32).cast(), &mut n) } == 0).then_some(v)
}

/// `Connect` de `socket_linux.cc`: a conexão em andamento conta como
/// sucesso.
#[cfg(unix)]
fn connect_socket(fd: i32, e: &OsAddress) -> IoResult<i64> {
    loop {
        // SAFETY: `e` é um `sockaddr` válido do tamanho dado.
        if unsafe { connect(fd, e.bytes.as_ptr(), e.family_size()) } == 0 {
            return Ok(i64::from(fd));
        }
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(c) if c == network_only::EINPROGRESS => return Ok(i64::from(fd)),
            _ if err.kind() == std::io::ErrorKind::Interrupted => continue,
            _ => {
                close_socket_descriptor(i64::from(fd));
                return Err(OsError::de(&err));
            }
        }
    }
}

/// `Socket::CreateConnect`.
#[cfg(unix)]
fn create_connection(e: &OsAddress) -> IoResult<i64> {
    let fd = create_socket(e.family(), network_only::SOCK_STREAM, 0)?;
    connect_socket(fd, e)
}

/// `Socket::CreateBindConnect`.
#[cfg(unix)]
fn create_connection_with_source(e: &OsAddress, origin: &OsAddress) -> IoResult<i64> {
    let fd = create_socket(e.family(), network_only::SOCK_STREAM, 0)?;
    // SAFETY: `origin` é um `sockaddr` válido.
    if unsafe { bind(fd, origin.bytes.as_ptr(), origin.family_size()) } != 0 {
        return Err(close_with_error(fd));
    }
    connect_socket(fd, e)
}

/// `Socket::CreateUnixDomainConnect` / `CreateUnixDomainBindConnect`.
#[cfg(unix)]
fn create_unix_connection(e: &OsAddress, origin: Option<&OsAddress>) -> IoResult<i64> {
    let fd = create_socket(network_only::AF_UNIX, network_only::SOCK_STREAM, 0)?;
    if let Some(o) = origin {
        // SAFETY: `o` é um `sockaddr_un` válido.
        if unsafe { bind(fd, o.bytes.as_ptr(), o.size) } != 0 {
            return Err(close_with_error(fd));
        }
    }
    // SAFETY: `e` é um `sockaddr_un` válido.
    if unsafe { connect(fd, e.bytes.as_ptr(), e.size) } == 0 {
        return Ok(i64::from(fd));
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(network_only::EAGAIN) {
        return Ok(i64::from(fd));
    }
    close_socket_descriptor(i64::from(fd));
    Err(OsError::de(&err))
}

/// `ServerSocket::CreateBindListen`: `SO_REUSEADDR`, `IPV6_V6ONLY` e, se o
/// sistema der a porta 65535 a um pedido de porta 0, outra tentativa (a
/// VM evita essa porta).
#[cfg(unix)]
fn create_listener(e: &OsAddress, queue: i64, v6_only: bool) -> IoResult<i64> {
    let fd = create_socket(e.family(), network_only::SOCK_STREAM, 0)?;
    set_int_option(fd, network_only::SOL_SOCKET, network_only::SO_REUSEADDR, 1);
    if e.is_ipv6() {
        set_int_option(fd, network_only::IPPROTO_IPV6, network_only::IPV6_V6ONLY, i32::from(v6_only));
    }
    // SAFETY: `e` é um `sockaddr` válido.
    if unsafe { bind(fd, e.bytes.as_ptr(), e.family_size()) } < 0 {
        return Err(close_with_error(fd));
    }
    if e.port() == 0 && local_port(i64::from(fd)) == 65535 {
        let new = create_listener(e, queue, v6_only);
        close_socket_descriptor(i64::from(fd));
        return new;
    }
    // SAFETY: descritor aberto.
    if unsafe { listen(fd, if queue > 0 { queue as i32 } else { network_only::SOMAXCONN }) } != 0 {
        return Err(close_with_error(fd));
    }
    Ok(i64::from(fd))
}

/// `ServerSocket::CreateUnixDomainBindListen`.
#[cfg(unix)]
fn create_unix_listener(e: &OsAddress, queue: i64) -> IoResult<i64> {
    let fd = create_socket(network_only::AF_UNIX, network_only::SOCK_STREAM, 0)?;
    // SAFETY: `e` é um `sockaddr_un` válido.
    if unsafe { bind(fd, e.bytes.as_ptr(), e.size) } < 0 {
        return Err(close_with_error(fd));
    }
    // SAFETY: descritor aberto.
    if unsafe { listen(fd, if queue > 0 { queue as i32 } else { network_only::SOMAXCONN }) } != 0 {
        return Err(close_with_error(fd));
    }
    Ok(i64::from(fd))
}

/// `ServerSocket::Accept`: o descritor novo (não bloqueante), -2 numa
/// falha temporária, -1 num erro.
#[cfg(unix)]
fn accept_conn(fd: i64) -> i64 {
    let mut e = OsAddress::empty();
    let mut n = 128u32;
    loop {
        // SAFETY: `e` tem 128 bytes.
        let new = unsafe { accept(fd as i32, e.bytes.as_mut_ptr(), &mut n) };
        if io_trace() {
            eprintln!(
                "[io] accept fd={fd} -> {new} {}",
                if new < 0 { std::io::Error::last_os_error().to_string() } else { String::new() }
            );
        }
        if new >= 0 {
            if !(close_on_exec(new) && set_nonblocking(new)) {
                close_socket_descriptor(i64::from(new));
                return -1;
            }
            #[cfg(not(target_os = "linux"))]
            set_int_option(new, network_only::SOL_SOCKET, network_only::SO_NOSIGPIPE, 1);
            return i64::from(new);
        }
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        return match err.raw_os_error() {
            Some(c) if network_only::TRANSIENT_ACCEPT_ERRORS.contains(&c) => -2,
            _ => -1,
        };
    }
}

/// `SocketBase::GetPort` (0 em erro).
#[cfg(unix)]
fn local_port(fd: i64) -> i64 {
    let mut e = OsAddress::empty();
    let mut n = 128u32;
    // SAFETY: `e` tem 128 bytes.
    if unsafe { getsockname(fd as i32, e.bytes.as_mut_ptr(), &mut n) } != 0 {
        return 0;
    }
    e.size = n;
    e.port()
}

/// `SocketBase::GetRemotePeer`: o endereço e a porta do outro lado.
#[cfg(unix)]
fn remote_peer(fd: i64) -> IoResult<(OsAddress, i64)> {
    let mut e = OsAddress::empty();
    let mut n = 128u32;
    // SAFETY: `e` tem 128 bytes.
    if unsafe { getpeername(fd as i32, e.bytes.as_mut_ptr(), &mut n) } != 0 {
        return Err(last_error());
    }
    e.size = n;
    let port = if n as usize <= 2 { 0 } else { e.port() };
    Ok((e, port))
}

/// `FDUtils::AvailableBytes` (`FIONREAD`).
#[cfg(unix)]
fn available_bytes(fd: i64) -> i64 {
    let mut n = 0i32;
    // SAFETY: `n` recebe o inteiro do `ioctl`.
    if unsafe { ioctl(fd as i32, network_only::FIONREAD, &mut n as *mut i32) } < 0 {
        return -1;
    }
    i64::from(n)
}

/// `SocketBase::Read` assíncrono: 0 bytes se não há o que ler agora.
#[cfg(unix)]
fn read_from_socket(fd: i64, dest: &mut [u8]) -> IoResult<usize> {
    loop {
        // SAFETY: `dest` é gravável com o tamanho dado.
        let n = unsafe { read(fd as i32, dest.as_mut_ptr().cast(), dest.len()) };
        if n >= 0 {
            return Ok(n as usize);
        }
        let e = std::io::Error::last_os_error();
        match e.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok(0),
            _ => return Err(OsError::de(&e)),
        }
    }
}

/// `SocketBase::Write`: os bytes escritos (0 se o soquete está cheio).
#[cfg(unix)]
fn write_to_socket(fd: i64, data: &[u8]) -> IoResult<usize> {
    loop {
        // SAFETY: `data` é legível com o tamanho dado.
        let n = unsafe { write(fd as i32, data.as_ptr().cast(), data.len()) };
        if n >= 0 {
            return Ok(n as usize);
        }
        let e = std::io::Error::last_os_error();
        match e.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok(0),
            _ => return Err(OsError::de(&e)),
        }
    }
}

/// `SocketBase::GetError` (`SO_ERROR`).
#[cfg(unix)]
fn pending_error(fd: i64) -> i32 {
    get_int_option(fd as i32, network_only::SOL_SOCKET, network_only::SO_ERROR).unwrap_or(0)
}

/// `SocketBase::LookupAddress` (`getaddrinfo`, primeiro com
/// `AI_ADDRCONFIG`): os endereços IPv4/IPv6 de `host`.
#[cfg(unix)]
fn resolve_name(host: &str, tipo: i64) -> IoResult<Vec<OsAddress>> {
    let Ok(c) = std::ffi::CString::new(host) else {
        return Err(OsError::from_code(network_only::EINVAL));
    };
    let family = match tipo {
        TYPE_IPV4 => i32::from(network_only::AF_INET),
        TYPE_IPV6 => i32::from(network_only::AF_INET6),
        TYPE_UNIX => i32::from(network_only::AF_UNIX),
        _ => 0,
    };
    let mut hints = EnderecoInfo {
        ai_flags: network_only::AI_ADDRCONFIG,
        ai_family: family,
        ai_socktype: network_only::SOCK_STREAM,
        ai_protocol: network_only::IPPROTO_TCP,
        ai_addrlen: 0,
        ai_addr: std::ptr::null(),
        ai_canonname: std::ptr::null(),
        ai_next: std::ptr::null_mut(),
    };
    let mut res: *mut EnderecoInfo = std::ptr::null_mut();
    // SAFETY: `hints` e `res` vivem durante as chamadas.
    let mut status = unsafe { getaddrinfo(c.as_ptr(), std::ptr::null(), &hints, &mut res) };
    if status != 0 {
        hints.ai_flags = 0;
        status = unsafe { getaddrinfo(c.as_ptr(), std::ptr::null(), &hints, &mut res) };
        if status != 0 {
            return Err(resolve_error(status));
        }
    }
    let mut out = Vec::new();
    let mut current = res;
    while !current.is_null() {
        // SAFETY: nó da lista devolvida por `getaddrinfo`.
        let i = unsafe { &*current };
        if i.ai_family == i32::from(network_only::AF_INET) || i.ai_family == i32::from(network_only::AF_INET6) {
            out.push(OsAddress::from_pointer(i.ai_addr, i.ai_addrlen));
        }
        current = i.ai_next;
    }
    // SAFETY: a lista veio de `getaddrinfo`.
    unsafe { freeaddrinfo(res) };
    Ok(out)
}

/// O `OSError` de `getaddrinfo` (`kGetAddressInfo`): o código e o
/// `gai_strerror`.
#[cfg(unix)]
fn resolve_error(status: i32) -> OsError {
    // SAFETY: `gai_strerror` devolve um texto estático.
    let m = unsafe { std::ffi::CStr::from_ptr(gai_strerror(status)) }.to_string_lossy().into_owned();
    OsError { code: i64::from(status), message: m }
}

/// `SocketBase::ReverseLookup`.
#[cfg(unix)]
fn address_name(e: &OsAddress) -> IoResult<String> {
    let mut b = [0 as std::ffi::c_char; 1025];
    // SAFETY: o buffer tem `NI_MAXHOST` bytes.
    let status = unsafe {
        getnameinfo(e.bytes.as_ptr(), e.family_size(), b.as_mut_ptr(), b.len() as u32, std::ptr::null_mut(), 0, network_only::NI_NAMEREQD)
    };
    if status != 0 {
        return Err(resolve_error(status));
    }
    // SAFETY: texto terminado em NUL.
    Ok(unsafe { std::ffi::CStr::from_ptr(b.as_ptr()) }.to_string_lossy().into_owned())
}

/// `SocketBase::ListInterfaces`: (endereço, nome da interface, índice).
#[cfg(unix)]
fn list_interfaces(tipo: i64) -> IoResult<Vec<(OsAddress, String, i64)>> {
    let mut list: *mut InterfaceAddress = std::ptr::null_mut();
    // SAFETY: `list` recebe a lista do sistema.
    if unsafe { getifaddrs(&mut list) } != 0 {
        return Err(last_error());
    }
    let mut out = Vec::new();
    let mut current = list;
    while !current.is_null() {
        // SAFETY: nó da lista de `getifaddrs`.
        let i = unsafe { &*current };
        if !i.ifa_addr.is_null() {
            let e = OsAddress::from_pointer(i.ifa_addr, 128);
            let f = e.family();
            let accepts = match tipo {
                TYPE_IPV4 => f == network_only::AF_INET,
                TYPE_IPV6 => f == network_only::AF_INET6,
                _ => f == network_only::AF_INET || f == network_only::AF_INET6,
            };
            if accepts {
                let mut e = e;
                e.size = e.family_size();
                // SAFETY: o nome é um texto C da lista.
                let name = unsafe { std::ffi::CStr::from_ptr(i.ifa_name) };
                // SAFETY: consulta sem efeitos.
                let ix = unsafe { if_nametoindex(i.ifa_name) };
                out.push((e, name.to_string_lossy().into_owned(), i64::from(ix)));
            }
        }
        current = i.ifa_next;
    }
    // SAFETY: a lista veio de `getifaddrs`.
    unsafe { freeifaddrs(list) };
    Ok(out)
}

/// `SocketBase::ParseAddress` (`inet_pton`): IPv4 sem `:`, IPv6 com.
#[cfg(unix)]
fn parse_address(text: &str) -> Option<Vec<u8>> {
    let c = std::ffi::CString::new(text).ok()?;
    let v6 = text.contains(':');
    let mut b = [0u8; 16];
    let family = if v6 { network_only::AF_INET6 } else { network_only::AF_INET };
    // SAFETY: `b` tem 16 bytes.
    (unsafe { inet_pton(i32::from(family), c.as_ptr(), b.as_mut_ptr()) } == 1).then(|| b[..if v6 { 16 } else { 4 }].to_vec())
}


/// `Socket::CreateBindDatagram`.
#[cfg(unix)]
fn create_datagram(e: &OsAddress, reuse_address: bool, reuse_port: bool, ttl: i64) -> IoResult<i64> {
    let fd = create_socket(e.family(), network_only::SOCK_DGRAM, network_only::IPPROTO_UDP)?;
    if reuse_address {
        set_int_option(fd, network_only::SOL_SOCKET, network_only::SO_REUSEADDR, 1);
    }
    if reuse_port && !set_int_option(fd, network_only::SOL_SOCKET, network_only::SO_REUSEPORT, 1) {
        eprintln!("Dart Socket ERROR: {}.", last_error().message);
    }
    let (level, name) = if e.is_ipv6() { (network_only::IPPROTO_IPV6, network_only::IPV6_MULTICAST_HOPS) } else { (network_only::IPPROTO_IP, network_only::IP_MULTICAST_TTL) };
    if !set_int_option(fd, level, name, ttl as i32) {
        return Err(close_with_error(fd));
    }
    // SAFETY: `e` é um `sockaddr` válido.
    if unsafe { bind(fd, e.bytes.as_ptr(), e.family_size()) } < 0 {
        return Err(close_with_error(fd));
    }
    Ok(i64::from(fd))
}

// ---------------------------------------------------------------------------
// Opções, dados e datagramas (a interface que os natives usam).

/// `EINPROGRESS` (`OSError_inProgressErrorCode`).
#[cfg(unix)]
fn in_progress_code() -> i64 {
    i64::from(network_only::EINPROGRESS)
}

/// `SocketBase::IsBindError`.
#[cfg(unix)]
fn is_bind_error(error: i64) -> bool {
    [network_only::EADDRINUSE, network_only::EADDRNOTAVAIL, network_only::EINVAL].contains(&(error as i32))
}

/// `Socket_GetOption`: 0 `TCP_NODELAY`, 1 laço de multicast, 2 TTL de
/// multicast, 4 `SO_BROADCAST`; `None` com o erro no `errno`.
#[cfg(unix)]
fn get_option(fd: i64, option: i64, v4: bool) -> Option<i64> {
    let fd = fd as i32;
    match option {
        0 => get_int_option(fd, network_only::IPPROTO_TCP, network_only::TCP_NODELAY).map(i64::from),
        1 => {
            let (level, name) = if v4 { (network_only::IPPROTO_IP, network_only::IP_MULTICAST_LOOP) } else { (network_only::IPPROTO_IPV6, network_only::IPV6_MULTICAST_LOOP) };
            get_byte_or_int_option(fd, level, name)
        }
        2 => {
            let (level, name) = if v4 { (network_only::IPPROTO_IP, network_only::IP_MULTICAST_TTL) } else { (network_only::IPPROTO_IPV6, network_only::IPV6_MULTICAST_HOPS) };
            get_byte_or_int_option(fd, level, name)
        }
        4 => get_int_option(fd, network_only::SOL_SOCKET, network_only::SO_BROADCAST).map(i64::from),
        _ => None,
    }
}

/// Opções de multicast IPv4 são um byte no macOS e um inteiro no Linux.
#[cfg(unix)]
fn get_byte_or_int_option(fd: i32, level: i32, name: i32) -> Option<i64> {
    let mut v = [0u8; 4];
    let mut n = 4u32;
    // SAFETY: `v` tem 4 bytes; o sistema diz quantos usou.
    if unsafe { getsockopt(fd, level, name, v.as_mut_ptr().cast(), &mut n) } != 0 {
        return None;
    }
    Some(if n == 1 { i64::from(v[0]) } else { i64::from(i32::from_ne_bytes(v)) })
}

/// `Socket_SetOption` (as mesmas opções de [`get_option`]).
#[cfg(unix)]
fn set_option(fd: i64, option: i64, v4: bool, value: i64) -> bool {
    let fd = fd as i32;
    let (level, name) = match option {
        0 => (network_only::IPPROTO_TCP, network_only::TCP_NODELAY),
        1 if v4 => (network_only::IPPROTO_IP, network_only::IP_MULTICAST_LOOP),
        1 => (network_only::IPPROTO_IPV6, network_only::IPV6_MULTICAST_LOOP),
        2 if v4 => (network_only::IPPROTO_IP, network_only::IP_MULTICAST_TTL),
        2 => (network_only::IPPROTO_IPV6, network_only::IPV6_MULTICAST_HOPS),
        4 => (network_only::SOL_SOCKET, network_only::SO_BROADCAST),
        _ => return false,
    };
    set_int_option(fd, level, name, value as i32)
}

/// `setsockopt` com os bytes de `data`.
#[cfg(unix)]
fn set_raw_option(fd: i64, level: i64, option: i64, data: &[u8]) -> bool {
    // SAFETY: `data` vive durante a chamada.
    unsafe { setsockopt(fd as i32, level as i32, option as i32, data.as_ptr().cast(), data.len() as u32) == 0 }
}

/// `getsockopt` nos bytes de `data` (encurtados ao tamanho devolvido).
#[cfg(unix)]
fn get_raw_option(fd: i64, level: i64, option: i64, data: &mut Vec<u8>) -> bool {
    let mut n = data.len() as u32;
    // SAFETY: `data` é gravável com `n` bytes.
    let ok = unsafe { getsockopt(fd as i32, level as i32, option as i32, data.as_mut_ptr().cast(), &mut n) } == 0;
    data.truncate(n as usize);
    ok
}

/// `RawSocketOption_GetOptionValue`: as constantes do sistema.
#[cfg(unix)]
fn raw_option_constant(key: i64) -> Option<i64> {
    Some(i64::from(match key {
        0 => network_only::SOL_SOCKET,
        1 => network_only::IPPROTO_IP,
        2 => network_only::IP_MULTICAST_IF,
        3 => network_only::IPPROTO_IPV6,
        4 => network_only::IPV6_MULTICAST_IF,
        5 => network_only::IPPROTO_TCP,
        6 => network_only::IPPROTO_UDP,
        _ => return None,
    }))
}

/// Os bytes prontos para leitura no soquete (-1 se o sistema não diz).
#[cfg(unix)]
fn available(s: &NativeSocket) -> i64 {
    available_bytes(s.descriptor())
}

/// Lê o que houver (0 sem dados agora).
#[cfg(unix)]
fn read_from(s: &NativeSocket, dest: &mut [u8]) -> IoResult<usize> {
    read_from_socket(s.descriptor(), dest)
}

/// Escreve o que couber (0 com o soquete cheio).
#[cfg(unix)]
fn write_at(s: &NativeSocket, data: &[u8]) -> IoResult<usize> {
    write_to_socket(s.descriptor(), data)
}

/// Se há um datagrama (espiando um byte).
#[cfg(unix)]
fn has_datagram(fd: i64) -> bool {
    let mut b = [0u8; 1];
    // SAFETY: `b` tem 1 byte.
    unsafe { recvfrom(fd as i32, b.as_mut_ptr(), 1, network_only::MSG_PEEK, std::ptr::null_mut(), std::ptr::null_mut()) >= 0 }
}

/// Recebe um datagrama (`None` se não há nenhum agora).
#[cfg(unix)]
fn receive_datagram(fd: i64, maximum: usize) -> IoResult<Option<(Vec<u8>, OsAddress)>> {
    let mut buf = vec![0u8; maximum];
    let mut e = OsAddress::empty();
    let mut n = 128u32;
    loop {
        // SAFETY: o buffer e o endereço são graváveis com os tamanhos dados.
        let r = unsafe { recvfrom(fd as i32, buf.as_mut_ptr(), buf.len(), 0, e.bytes.as_mut_ptr(), &mut n) };
        if r >= 0 {
            buf.truncate(r as usize);
            e.size = n;
            return Ok(Some((buf, e)));
        }
        let err = std::io::Error::last_os_error();
        match err.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok(None),
            _ => return Err(OsError::de(&err)),
        }
    }
}

/// Envia um datagrama; 0 se o soquete está cheio.
#[cfg(unix)]
fn send_datagram(fd: i64, data: &[u8], e: &OsAddress) -> IoResult<usize> {
    loop {
        // SAFETY: os dados e o endereço são válidos.
        let r = unsafe { sendto(fd as i32, data.as_ptr(), data.len(), 0, e.bytes.as_ptr(), e.family_size()) };
        if r >= 0 {
            return Ok(r as usize);
        }
        let err = std::io::Error::last_os_error();
        match err.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok(0),
            _ => return Err(OsError::de(&err)),
        }
    }
}

/// `SocketBase::HasPendingWrite`: só o Windows tem escritas em andamento.
#[cfg(unix)]
fn pending_write(_s: &NativeSocket) -> bool {
    false
}

/// O tipo do descritor (0 terminal, 1 pipe, 2 arquivo, 4 outro).
#[cfg(unix)]
fn descriptor_type(fd: i64) -> IoResult<i64> {
    use std::os::unix::fs::FileTypeExt;
    let f = std::mem::ManuallyDrop::new(file_from_descriptor(fd));
    let t = f.metadata()?.file_type();
    Ok(if t.is_char_device() {
        0
    } else if t.is_fifo() {
        1
    } else if t.is_file() {
        2
    } else {
        4
    })
}


/// Apaga o arquivo de um soquete de domínio Unix (o último uso de um
/// soquete de escuta fechou; o nome abstrato do Linux, com NUL na frente,
/// não tem arquivo).
#[cfg(unix)]
fn delete_unix_path(file_path: &[u8]) {
    if file_path.first() == Some(&0) {
        return;
    }
    if let Ok(c) = std::ffi::CString::new(file_path) {
        // SAFETY: caminho C válido.
        unsafe { unlink(c.as_ptr()) };
    }
}

// ---------------------------------------------------------------------------
// Mensagens de controle (`SocketBase::SendMessage`/`ReceiveMessage`).

/// Uma mensagem de controle: nível, tipo e dados.
#[cfg(unix)]
type ControlMessage = (i64, i64, Vec<u8>);

/// O tamanho do buffer de controle da recepção (cabe o que o sistema
/// entrega de uma vez: `SCM_MAX_FD` do Linux é 253 descritores).
#[cfg(unix)]
const RECEIVED_CONTROL_SIZE: usize = 4096;

/// O `SOL_SOCKET`/`SCM_RIGHTS` do sistema (a mensagem que leva descritores).
#[cfg(unix)]
fn resource_level_and_type() -> (i64, i64) {
    (i64::from(network_only::SOL_SOCKET), i64::from(network_only::SCM_RIGHTS))
}

/// `SocketBase::SendMessage`: os bytes enviados (0 se o soquete está cheio).
#[cfg(unix)]
fn send_message(fd: i64, data: &[u8], controls: &[ControlMessage]) -> IoResult<usize> {
    let size: usize = controls.iter().map(|(_, _, d)| align_cmsg(cmsg::TAMANHO_DO_CABECALHO + d.len())).sum();
    // `u64` para o alinhamento do `cmsghdr`.
    let mut control = vec![0u64; size.div_ceil(8)];
    // SAFETY: a fatia cobre o `Vec` de `u64`, com `size` ≤ o comprimento.
    let bytes = unsafe { std::slice::from_raw_parts_mut(control.as_mut_ptr().cast::<u8>(), control.len() * 8) };
    let mut p = 0;
    for (level, tipo, d) in controls {
        let b = &mut bytes[p..];
        cmsg::write_size(b, cmsg::TAMANHO_DO_CABECALHO + d.len());
        let n = cmsg::LEVEL_SHIFT;
        b[n..n + 4].copy_from_slice(&(*level as i32).to_ne_bytes());
        b[n + 4..n + 8].copy_from_slice(&(*tipo as i32).to_ne_bytes());
        b[cmsg::TAMANHO_DO_CABECALHO..cmsg::TAMANHO_DO_CABECALHO + d.len()].copy_from_slice(d);
        p += align_cmsg(cmsg::TAMANHO_DO_CABECALHO + d.len());
    }
    let mut piece = Slice { base: data.as_ptr().cast_mut(), size: data.len() };
    let m = CabecalhoDeMensagem {
        name: std::ptr::null_mut(),
        name_size: 0,
        slices: &mut piece,
        n_slices: 1,
        control: if size == 0 { std::ptr::null_mut() } else { bytes.as_mut_ptr() },
        control_size: size as _,
        flag_bits: 0,
    };
    loop {
        // SAFETY: `m` aponta para a fatia e o controle vivos; `sendmsg` só lê.
        let r = unsafe { sendmsg(fd as i32, &m, network_only::MSG_NOSIGNAL) };
        if r >= 0 {
            return Ok(r as usize);
        }
        let err = std::io::Error::last_os_error();
        match err.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok(0),
            _ => return Err(OsError::de(&err)),
        }
    }
}

/// `SocketBase::ReceiveMessage`: os dados (vazios se não há nada agora) e as
/// mensagens de controle recebidas.
#[cfg(unix)]
fn receive_message(fd: i64, maximum: usize) -> IoResult<(Vec<u8>, Vec<ControlMessage>)> {
    let mut data = vec![0u8; maximum];
    let mut control = vec![0u64; RECEIVED_CONTROL_SIZE / 8];
    let mut piece = Slice { base: data.as_mut_ptr(), size: data.len() };
    let mut m = CabecalhoDeMensagem {
        name: std::ptr::null_mut(),
        name_size: 0,
        slices: &mut piece,
        n_slices: 1,
        control: control.as_mut_ptr().cast(),
        control_size: RECEIVED_CONTROL_SIZE as _,
        flag_bits: 0,
    };
    let read_list = loop {
        // SAFETY: `m` aponta para os buffers graváveis com os tamanhos dados.
        let r = unsafe { recvmsg(fd as i32, &mut m, network_only::MSG_CMSG_CLOEXEC) };
        if r >= 0 {
            break r as usize;
        }
        let err = std::io::Error::last_os_error();
        match err.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok((Vec::new(), Vec::new())),
            _ => return Err(OsError::de(&err)),
        }
    };
    data.truncate(read_list);
    // SAFETY: o sistema preencheu `control_size` bytes do buffer.
    let bytes = unsafe { std::slice::from_raw_parts(control.as_ptr().cast::<u8>(), (m.control_size as usize).min(RECEIVED_CONTROL_SIZE)) };
    let mut messages = Vec::new();
    let mut p = 0;
    while p + cmsg::TAMANHO_DO_CABECALHO <= bytes.len() {
        let b = &bytes[p..];
        let n = cmsg::read_size(b);
        if n < cmsg::TAMANHO_DO_CABECALHO || n > b.len() {
            break;
        }
        let d = cmsg::LEVEL_SHIFT;
        let level = i32::from_ne_bytes(b[d..d + 4].try_into().unwrap());
        let tipo = i32::from_ne_bytes(b[d + 4..d + 8].try_into().unwrap());
        messages.push((i64::from(level), i64::from(tipo), b[cmsg::TAMANHO_DO_CABECALHO..n].to_vec()));
        p += align_cmsg(n);
    }
    Ok((data, messages))
}

/// O endereço local do descritor (`SocketBase::GetSocketName`).
#[cfg(unix)]
fn local_address(fd: i64) -> IoResult<OsAddress> {
    let mut e = OsAddress::empty();
    let mut n = 128u32;
    // SAFETY: `e` tem 128 bytes.
    if unsafe { getsockname(fd as i32, e.bytes.as_mut_ptr(), &mut n) } != 0 {
        return Err(last_error());
    }
    e.size = n;
    Ok(e)
}
