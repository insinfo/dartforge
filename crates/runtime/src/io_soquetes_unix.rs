// Runtime nativo: a camada Unix (Linux e macOS) dos soquetes do `dart:io`
// (`socket_base_posix.cc`, `socket_linux.cc`, `socket_macos.cc` da VM): as
// constantes e estruturas do sistema, os endereços e as operações que os
// natives de `io_soquetes.rs` chamam. A camada do Windows, com os mesmos
// nomes, está em `io_windows_soquetes.rs`.
// ---------------------------------------------------------------------------
// Constantes do sistema.

#[cfg(target_os = "linux")]
mod so_rede {
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
    pub const TAMANHO_DO_CAMINHO_UNIX: usize = 108;
    /// Erros de `accept` que não fecham o servidor (`IsTemporaryAcceptError`).
    pub const ERROS_TEMPORARIOS_DE_ACCEPT: &[i32] = &[11, 100, 71, 92, 112, 64, 113, 95, 101];
}

#[cfg(all(unix, not(target_os = "linux")))]
mod so_rede {
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
    pub const TAMANHO_DO_CAMINHO_UNIX: usize = 104;
    pub const ERROS_TEMPORARIOS_DE_ACCEPT: &[i32] = &[35, 50, 100, 42, 64, 65, 102, 51];
}

#[cfg(unix)]
unsafe extern "C" {
    fn socket(dominio: i32, tipo: i32, protocolo: i32) -> i32;
    fn connect(fd: i32, e: *const u8, n: u32) -> i32;
    fn bind(fd: i32, e: *const u8, n: u32) -> i32;
    fn listen(fd: i32, fila: i32) -> i32;
    fn accept(fd: i32, e: *mut u8, n: *mut u32) -> i32;
    fn getsockname(fd: i32, e: *mut u8, n: *mut u32) -> i32;
    fn getpeername(fd: i32, e: *mut u8, n: *mut u32) -> i32;
    fn setsockopt(fd: i32, nivel: i32, opcao: i32, v: *const std::ffi::c_void, n: u32) -> i32;
    fn getsockopt(fd: i32, nivel: i32, opcao: i32, v: *mut std::ffi::c_void, n: *mut u32) -> i32;
    fn recvfrom(fd: i32, b: *mut u8, n: usize, flags: i32, e: *mut u8, en: *mut u32) -> isize;
    fn sendto(fd: i32, b: *const u8, n: usize, flags: i32, e: *const u8, en: u32) -> isize;
    fn write(fd: i32, b: *const std::ffi::c_void, n: usize) -> isize;
    fn getnameinfo(e: *const u8, en: u32, host: *mut std::ffi::c_char, hn: u32, serv: *mut std::ffi::c_char, sn: u32, flags: i32) -> i32;
    fn getaddrinfo(host: *const std::ffi::c_char, serv: *const std::ffi::c_char, dicas: *const EnderecoInfo, res: *mut *mut EnderecoInfo) -> i32;
    fn freeaddrinfo(res: *mut EnderecoInfo);
    fn gai_strerror(codigo: i32) -> *const std::ffi::c_char;
    fn inet_pton(familia: i32, texto: *const std::ffi::c_char, destino: *mut u8) -> i32;
    fn getifaddrs(res: *mut *mut EnderecoDeInterface) -> i32;
    fn freeifaddrs(res: *mut EnderecoDeInterface);
    fn if_nametoindex(nome: *const std::ffi::c_char) -> u32;
    fn unlink(caminho: *const std::ffi::c_char) -> i32;
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
struct EnderecoDeInterface {
    ifa_next: *mut EnderecoDeInterface,
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
impl EnderecoSo {
    fn vazio() -> EnderecoSo {
        EnderecoSo { bytes: [0; 128], tamanho: 128 }
    }

    /// A família (`sa_family`; no macOS vem depois do `sa_len`).
    fn familia(&self) -> u16 {
        if cfg!(target_os = "linux") {
            u16::from_ne_bytes([self.bytes[0], self.bytes[1]])
        } else {
            u16::from(self.bytes[1])
        }
    }

    fn gravar_familia(&mut self, f: u16, tamanho: u32) {
        if cfg!(target_os = "linux") {
            self.bytes[..2].copy_from_slice(&f.to_ne_bytes());
        } else {
            self.bytes[0] = tamanho as u8;
            self.bytes[1] = f as u8;
        }
        self.tamanho = tamanho;
    }

    /// `SocketAddress::GetSockAddr`: 4 bytes → IPv4, 16 → IPv6.
    fn de_ip(ip: &[u8]) -> Option<EnderecoSo> {
        let mut e = EnderecoSo::vazio();
        match ip.len() {
            4 => {
                e.gravar_familia(so_rede::AF_INET, 16);
                e.bytes[4..8].copy_from_slice(ip);
            }
            16 => {
                e.gravar_familia(so_rede::AF_INET6, 28);
                e.bytes[8..24].copy_from_slice(ip);
            }
            _ => return None,
        }
        Some(e)
    }

    /// Um endereço de domínio Unix (`sockaddr_un`).
    fn de_caminho_unix(caminho: &[u8]) -> ResultadoIo<EnderecoSo> {
        if caminho.len() >= so_rede::TAMANHO_DO_CAMINHO_UNIX {
            return Err(ErroDoSo::do_codigo(so_rede::EINVAL));
        }
        let mut e = EnderecoSo::vazio();
        e.gravar_familia(so_rede::AF_UNIX, (2 + caminho.len() + 1) as u32);
        e.bytes[2..2 + caminho.len()].copy_from_slice(caminho);
        Ok(e)
    }

    fn de_ponteiro(p: *const u8, n: u32) -> EnderecoSo {
        let mut e = EnderecoSo::vazio();
        let n = (n as usize).min(128);
        // SAFETY: `p` aponta para um `sockaddr` de `n` bytes do sistema.
        e.bytes[..n].copy_from_slice(unsafe { std::slice::from_raw_parts(p, n) });
        e.tamanho = n as u32;
        e
    }

    fn e_ipv6(&self) -> bool {
        self.familia() == so_rede::AF_INET6
    }

    /// `SocketAddress::GetType`.
    fn tipo(&self) -> i64 {
        match self.familia() {
            so_rede::AF_INET => TIPO_IPV4,
            so_rede::AF_INET6 => TIPO_IPV6,
            _ => TIPO_UNIX,
        }
    }

    fn porta(&self) -> i64 {
        match self.familia() {
            so_rede::AF_INET | so_rede::AF_INET6 => i64::from(u16::from_be_bytes([self.bytes[2], self.bytes[3]])),
            _ => 0,
        }
    }

    fn gravar_porta(&mut self, p: i64) {
        self.bytes[2..4].copy_from_slice(&(p as u16).to_be_bytes());
    }

    fn escopo(&self) -> i64 {
        if self.e_ipv6() { i64::from(u32::from_ne_bytes(self.bytes[24..28].try_into().expect("4 bytes"))) } else { 0 }
    }

    fn gravar_escopo(&mut self, s: i64) {
        if self.e_ipv6() {
            self.bytes[24..28].copy_from_slice(&(s as u32).to_ne_bytes());
        }
    }

    /// Os bytes do endereço IP (`SocketAddress::ToTypedData`).
    fn ip(&self) -> Vec<u8> {
        if self.e_ipv6() { self.bytes[8..24].to_vec() } else { self.bytes[4..8].to_vec() }
    }

    /// O tamanho do `sockaddr` da família (`GetAddrLength`).
    fn tamanho_da_familia(&self) -> u32 {
        match self.familia() {
            so_rede::AF_INET => 16,
            so_rede::AF_INET6 => 28,
            _ => self.tamanho,
        }
    }

    /// O texto numérico (`getnameinfo(NI_NUMERICHOST)`, com `%interface` no
    /// IPv6 de escopo local); o caminho, no domínio Unix.
    fn texto(&self) -> String {
        if self.familia() == so_rede::AF_UNIX {
            let caminho = &self.bytes[2..self.tamanho.max(2) as usize];
            let fim = caminho.iter().position(|&c| c == 0).unwrap_or(caminho.len());
            return String::from_utf8_lossy(&caminho[..fim]).into_owned();
        }
        let mut b = [0 as std::ffi::c_char; 64];
        // SAFETY: o buffer tem 64 bytes (INET6_ADDRSTRLEN com escopo).
        let r = unsafe {
            getnameinfo(self.bytes.as_ptr(), self.tamanho_da_familia(), b.as_mut_ptr(), b.len() as u32, std::ptr::null_mut(), 0, so_rede::NI_NUMERICHOST)
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
fn ultimo_erro() -> ErroDoSo {
    ErroDoSo::de(&std::io::Error::last_os_error())
}

/// Fecha `fd` preservando o erro corrente (`FDUtils::SaveErrorAndClose`).
#[cfg(unix)]
fn fechar_com_erro(fd: i32) -> ErroDoSo {
    let e = ultimo_erro();
    fechar_descritor_de_soquete(i64::from(fd));
    e
}

/// Um soquete não bloqueante e fechado no `exec`.
#[cfg(unix)]
fn criar_soquete(familia: u16, tipo: i32, protocolo: i32) -> ResultadoIo<i32> {
    #[cfg(target_os = "linux")]
    let fd = {
        const SOCK_NONBLOCK: i32 = 0o4000;
        const SOCK_CLOEXEC: i32 = 0o2000000;
        // SAFETY: cria um soquete.
        unsafe { socket(i32::from(familia), tipo | SOCK_NONBLOCK | SOCK_CLOEXEC, protocolo) }
    };
    #[cfg(not(target_os = "linux"))]
    let fd = {
        // SAFETY: cria um soquete.
        let fd = unsafe { socket(i32::from(familia), tipo, protocolo) };
        if fd >= 0 && !(fechar_no_exec(fd) && tornar_nao_bloqueante(fd)) {
            return Err(fechar_com_erro(fd));
        }
        if fd >= 0 {
            // Sem SIGPIPE ao escrever num soquete fechado (a VM do macOS faz
            // o mesmo).
            definir_opcao_int(fd, so_rede::SOL_SOCKET, so_rede::SO_NOSIGPIPE, 1);
        }
        fd
    };
    if fd < 0 {
        return Err(ultimo_erro());
    }
    Ok(fd)
}

#[cfg(unix)]
fn definir_opcao_int(fd: i32, nivel: i32, opcao: i32, valor: i32) -> bool {
    // SAFETY: `valor` vive durante a chamada.
    unsafe { setsockopt(fd, nivel, opcao, (&valor as *const i32).cast(), 4) == 0 }
}

#[cfg(unix)]
fn ler_opcao_int(fd: i32, nivel: i32, opcao: i32) -> Option<i32> {
    let mut v = 0i32;
    let mut n = 4u32;
    // SAFETY: `v` tem 4 bytes.
    (unsafe { getsockopt(fd, nivel, opcao, (&mut v as *mut i32).cast(), &mut n) } == 0).then_some(v)
}

/// `Connect` de `socket_linux.cc`: a conexão em andamento conta como
/// sucesso.
#[cfg(unix)]
fn conectar(fd: i32, e: &EnderecoSo) -> ResultadoIo<i64> {
    loop {
        // SAFETY: `e` é um `sockaddr` válido do tamanho dado.
        if unsafe { connect(fd, e.bytes.as_ptr(), e.tamanho_da_familia()) } == 0 {
            return Ok(i64::from(fd));
        }
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(c) if c == so_rede::EINPROGRESS => return Ok(i64::from(fd)),
            _ if err.kind() == std::io::ErrorKind::Interrupted => continue,
            _ => {
                fechar_descritor_de_soquete(i64::from(fd));
                return Err(ErroDoSo::de(&err));
            }
        }
    }
}

/// `Socket::CreateConnect`.
#[cfg(unix)]
fn criar_conexao(e: &EnderecoSo) -> ResultadoIo<i64> {
    let fd = criar_soquete(e.familia(), so_rede::SOCK_STREAM, 0)?;
    conectar(fd, e)
}

/// `Socket::CreateBindConnect`.
#[cfg(unix)]
fn criar_conexao_com_origem(e: &EnderecoSo, origem: &EnderecoSo) -> ResultadoIo<i64> {
    let fd = criar_soquete(e.familia(), so_rede::SOCK_STREAM, 0)?;
    // SAFETY: `origem` é um `sockaddr` válido.
    if unsafe { bind(fd, origem.bytes.as_ptr(), origem.tamanho_da_familia()) } != 0 {
        return Err(fechar_com_erro(fd));
    }
    conectar(fd, e)
}

/// `Socket::CreateUnixDomainConnect` / `CreateUnixDomainBindConnect`.
#[cfg(unix)]
fn criar_conexao_unix(e: &EnderecoSo, origem: Option<&EnderecoSo>) -> ResultadoIo<i64> {
    let fd = criar_soquete(so_rede::AF_UNIX, so_rede::SOCK_STREAM, 0)?;
    if let Some(o) = origem {
        // SAFETY: `o` é um `sockaddr_un` válido.
        if unsafe { bind(fd, o.bytes.as_ptr(), o.tamanho) } != 0 {
            return Err(fechar_com_erro(fd));
        }
    }
    // SAFETY: `e` é um `sockaddr_un` válido.
    if unsafe { connect(fd, e.bytes.as_ptr(), e.tamanho) } == 0 {
        return Ok(i64::from(fd));
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(so_rede::EAGAIN) {
        return Ok(i64::from(fd));
    }
    fechar_descritor_de_soquete(i64::from(fd));
    Err(ErroDoSo::de(&err))
}

/// `ServerSocket::CreateBindListen`: `SO_REUSEADDR`, `IPV6_V6ONLY` e, se o
/// sistema der a porta 65535 a um pedido de porta 0, outra tentativa (a
/// VM evita essa porta).
#[cfg(unix)]
fn criar_escuta(e: &EnderecoSo, fila: i64, so_v6: bool) -> ResultadoIo<i64> {
    let fd = criar_soquete(e.familia(), so_rede::SOCK_STREAM, 0)?;
    definir_opcao_int(fd, so_rede::SOL_SOCKET, so_rede::SO_REUSEADDR, 1);
    if e.e_ipv6() {
        definir_opcao_int(fd, so_rede::IPPROTO_IPV6, so_rede::IPV6_V6ONLY, i32::from(so_v6));
    }
    // SAFETY: `e` é um `sockaddr` válido.
    if unsafe { bind(fd, e.bytes.as_ptr(), e.tamanho_da_familia()) } < 0 {
        return Err(fechar_com_erro(fd));
    }
    if e.porta() == 0 && porta_local(i64::from(fd)) == 65535 {
        let novo = criar_escuta(e, fila, so_v6);
        fechar_descritor_de_soquete(i64::from(fd));
        return novo;
    }
    // SAFETY: descritor aberto.
    if unsafe { listen(fd, if fila > 0 { fila as i32 } else { so_rede::SOMAXCONN }) } != 0 {
        return Err(fechar_com_erro(fd));
    }
    Ok(i64::from(fd))
}

/// `ServerSocket::CreateUnixDomainBindListen`.
#[cfg(unix)]
fn criar_escuta_unix(e: &EnderecoSo, fila: i64) -> ResultadoIo<i64> {
    let fd = criar_soquete(so_rede::AF_UNIX, so_rede::SOCK_STREAM, 0)?;
    // SAFETY: `e` é um `sockaddr_un` válido.
    if unsafe { bind(fd, e.bytes.as_ptr(), e.tamanho) } < 0 {
        return Err(fechar_com_erro(fd));
    }
    // SAFETY: descritor aberto.
    if unsafe { listen(fd, if fila > 0 { fila as i32 } else { so_rede::SOMAXCONN }) } != 0 {
        return Err(fechar_com_erro(fd));
    }
    Ok(i64::from(fd))
}

/// `ServerSocket::Accept`: o descritor novo (não bloqueante), -2 numa
/// falha temporária, -1 num erro.
#[cfg(unix)]
fn aceitar(fd: i64) -> i64 {
    let mut e = EnderecoSo::vazio();
    let mut n = 128u32;
    loop {
        // SAFETY: `e` tem 128 bytes.
        let novo = unsafe { accept(fd as i32, e.bytes.as_mut_ptr(), &mut n) };
        if novo >= 0 {
            if !(fechar_no_exec(novo) && tornar_nao_bloqueante(novo)) {
                fechar_descritor_de_soquete(i64::from(novo));
                return -1;
            }
            #[cfg(not(target_os = "linux"))]
            definir_opcao_int(novo, so_rede::SOL_SOCKET, so_rede::SO_NOSIGPIPE, 1);
            return i64::from(novo);
        }
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        return match err.raw_os_error() {
            Some(c) if so_rede::ERROS_TEMPORARIOS_DE_ACCEPT.contains(&c) => -2,
            _ => -1,
        };
    }
}

/// `SocketBase::GetPort` (0 em erro).
#[cfg(unix)]
fn porta_local(fd: i64) -> i64 {
    let mut e = EnderecoSo::vazio();
    let mut n = 128u32;
    // SAFETY: `e` tem 128 bytes.
    if unsafe { getsockname(fd as i32, e.bytes.as_mut_ptr(), &mut n) } != 0 {
        return 0;
    }
    e.tamanho = n;
    e.porta()
}

/// `SocketBase::GetRemotePeer`: o endereço e a porta do outro lado.
#[cfg(unix)]
fn par_remoto(fd: i64) -> ResultadoIo<(EnderecoSo, i64)> {
    let mut e = EnderecoSo::vazio();
    let mut n = 128u32;
    // SAFETY: `e` tem 128 bytes.
    if unsafe { getpeername(fd as i32, e.bytes.as_mut_ptr(), &mut n) } != 0 {
        return Err(ultimo_erro());
    }
    e.tamanho = n;
    let porta = if n as usize <= 2 { 0 } else { e.porta() };
    Ok((e, porta))
}

/// `FDUtils::AvailableBytes` (`FIONREAD`).
#[cfg(unix)]
fn bytes_disponiveis(fd: i64) -> i64 {
    let mut n = 0i32;
    // SAFETY: `n` recebe o inteiro do `ioctl`.
    if unsafe { ioctl(fd as i32, so_rede::FIONREAD, &mut n as *mut i32) } < 0 {
        return -1;
    }
    i64::from(n)
}

/// `SocketBase::Read` assíncrono: 0 bytes se não há o que ler agora.
#[cfg(unix)]
fn ler_do_soquete(fd: i64, destino: &mut [u8]) -> ResultadoIo<usize> {
    loop {
        // SAFETY: `destino` é gravável com o tamanho dado.
        let n = unsafe { read(fd as i32, destino.as_mut_ptr().cast(), destino.len()) };
        if n >= 0 {
            return Ok(n as usize);
        }
        let e = std::io::Error::last_os_error();
        match e.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok(0),
            _ => return Err(ErroDoSo::de(&e)),
        }
    }
}

/// `SocketBase::Write`: os bytes escritos (0 se o soquete está cheio).
#[cfg(unix)]
fn escrever_no_soquete(fd: i64, dados: &[u8]) -> ResultadoIo<usize> {
    loop {
        // SAFETY: `dados` é legível com o tamanho dado.
        let n = unsafe { write(fd as i32, dados.as_ptr().cast(), dados.len()) };
        if n >= 0 {
            return Ok(n as usize);
        }
        let e = std::io::Error::last_os_error();
        match e.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok(0),
            _ => return Err(ErroDoSo::de(&e)),
        }
    }
}

/// `SocketBase::GetError` (`SO_ERROR`).
#[cfg(unix)]
fn erro_pendente(fd: i64) -> i32 {
    ler_opcao_int(fd as i32, so_rede::SOL_SOCKET, so_rede::SO_ERROR).unwrap_or(0)
}

/// `SocketBase::LookupAddress` (`getaddrinfo`, primeiro com
/// `AI_ADDRCONFIG`): os endereços IPv4/IPv6 de `host`.
#[cfg(unix)]
fn resolver_nome(host: &str, tipo: i64) -> ResultadoIo<Vec<EnderecoSo>> {
    let Ok(c) = std::ffi::CString::new(host) else {
        return Err(ErroDoSo::do_codigo(so_rede::EINVAL));
    };
    let familia = match tipo {
        TIPO_IPV4 => i32::from(so_rede::AF_INET),
        TIPO_IPV6 => i32::from(so_rede::AF_INET6),
        TIPO_UNIX => i32::from(so_rede::AF_UNIX),
        _ => 0,
    };
    let mut dicas = EnderecoInfo {
        ai_flags: so_rede::AI_ADDRCONFIG,
        ai_family: familia,
        ai_socktype: so_rede::SOCK_STREAM,
        ai_protocol: so_rede::IPPROTO_TCP,
        ai_addrlen: 0,
        ai_addr: std::ptr::null(),
        ai_canonname: std::ptr::null(),
        ai_next: std::ptr::null_mut(),
    };
    let mut res: *mut EnderecoInfo = std::ptr::null_mut();
    // SAFETY: `dicas` e `res` vivem durante as chamadas.
    let mut status = unsafe { getaddrinfo(c.as_ptr(), std::ptr::null(), &dicas, &mut res) };
    if status != 0 {
        dicas.ai_flags = 0;
        status = unsafe { getaddrinfo(c.as_ptr(), std::ptr::null(), &dicas, &mut res) };
        if status != 0 {
            return Err(erro_de_resolucao(status));
        }
    }
    let mut saida = Vec::new();
    let mut atual = res;
    while !atual.is_null() {
        // SAFETY: nó da lista devolvida por `getaddrinfo`.
        let i = unsafe { &*atual };
        if i.ai_family == i32::from(so_rede::AF_INET) || i.ai_family == i32::from(so_rede::AF_INET6) {
            saida.push(EnderecoSo::de_ponteiro(i.ai_addr, i.ai_addrlen));
        }
        atual = i.ai_next;
    }
    // SAFETY: a lista veio de `getaddrinfo`.
    unsafe { freeaddrinfo(res) };
    Ok(saida)
}

/// O `OSError` de `getaddrinfo` (`kGetAddressInfo`): o código e o
/// `gai_strerror`.
#[cfg(unix)]
fn erro_de_resolucao(status: i32) -> ErroDoSo {
    // SAFETY: `gai_strerror` devolve um texto estático.
    let m = unsafe { std::ffi::CStr::from_ptr(gai_strerror(status)) }.to_string_lossy().into_owned();
    ErroDoSo { codigo: i64::from(status), mensagem: m }
}

/// `SocketBase::ReverseLookup`.
#[cfg(unix)]
fn nome_do_endereco(e: &EnderecoSo) -> ResultadoIo<String> {
    let mut b = [0 as std::ffi::c_char; 1025];
    // SAFETY: o buffer tem `NI_MAXHOST` bytes.
    let status = unsafe {
        getnameinfo(e.bytes.as_ptr(), e.tamanho_da_familia(), b.as_mut_ptr(), b.len() as u32, std::ptr::null_mut(), 0, so_rede::NI_NAMEREQD)
    };
    if status != 0 {
        return Err(erro_de_resolucao(status));
    }
    // SAFETY: texto terminado em NUL.
    Ok(unsafe { std::ffi::CStr::from_ptr(b.as_ptr()) }.to_string_lossy().into_owned())
}

/// `SocketBase::ListInterfaces`: (endereço, nome da interface, índice).
#[cfg(unix)]
fn listar_interfaces(tipo: i64) -> ResultadoIo<Vec<(EnderecoSo, String, i64)>> {
    let mut lista: *mut EnderecoDeInterface = std::ptr::null_mut();
    // SAFETY: `lista` recebe a lista do sistema.
    if unsafe { getifaddrs(&mut lista) } != 0 {
        return Err(ultimo_erro());
    }
    let mut saida = Vec::new();
    let mut atual = lista;
    while !atual.is_null() {
        // SAFETY: nó da lista de `getifaddrs`.
        let i = unsafe { &*atual };
        if !i.ifa_addr.is_null() {
            let e = EnderecoSo::de_ponteiro(i.ifa_addr, 128);
            let f = e.familia();
            let aceita = match tipo {
                TIPO_IPV4 => f == so_rede::AF_INET,
                TIPO_IPV6 => f == so_rede::AF_INET6,
                _ => f == so_rede::AF_INET || f == so_rede::AF_INET6,
            };
            if aceita {
                let mut e = e;
                e.tamanho = e.tamanho_da_familia();
                // SAFETY: o nome é um texto C da lista.
                let nome = unsafe { std::ffi::CStr::from_ptr(i.ifa_name) };
                // SAFETY: consulta sem efeitos.
                let indice = unsafe { if_nametoindex(i.ifa_name) };
                saida.push((e, nome.to_string_lossy().into_owned(), i64::from(indice)));
            }
        }
        atual = i.ifa_next;
    }
    // SAFETY: a lista veio de `getifaddrs`.
    unsafe { freeifaddrs(lista) };
    Ok(saida)
}

/// `SocketBase::ParseAddress` (`inet_pton`): IPv4 sem `:`, IPv6 com.
#[cfg(unix)]
fn interpretar_endereco(texto: &str) -> Option<Vec<u8>> {
    let c = std::ffi::CString::new(texto).ok()?;
    let v6 = texto.contains(':');
    let mut b = [0u8; 16];
    let familia = if v6 { so_rede::AF_INET6 } else { so_rede::AF_INET };
    // SAFETY: `b` tem 16 bytes.
    (unsafe { inet_pton(i32::from(familia), c.as_ptr(), b.as_mut_ptr()) } == 1).then(|| b[..if v6 { 16 } else { 4 }].to_vec())
}


/// `Socket::CreateBindDatagram`.
#[cfg(unix)]
fn criar_datagrama(e: &EnderecoSo, reusar_endereco: bool, reusar_porta: bool, ttl: i64) -> ResultadoIo<i64> {
    let fd = criar_soquete(e.familia(), so_rede::SOCK_DGRAM, so_rede::IPPROTO_UDP)?;
    if reusar_endereco {
        definir_opcao_int(fd, so_rede::SOL_SOCKET, so_rede::SO_REUSEADDR, 1);
    }
    if reusar_porta && !definir_opcao_int(fd, so_rede::SOL_SOCKET, so_rede::SO_REUSEPORT, 1) {
        eprintln!("Dart Socket ERROR: {}.", ultimo_erro().mensagem);
    }
    let (nivel, nome) = if e.e_ipv6() { (so_rede::IPPROTO_IPV6, so_rede::IPV6_MULTICAST_HOPS) } else { (so_rede::IPPROTO_IP, so_rede::IP_MULTICAST_TTL) };
    if !definir_opcao_int(fd, nivel, nome, ttl as i32) {
        return Err(fechar_com_erro(fd));
    }
    // SAFETY: `e` é um `sockaddr` válido.
    if unsafe { bind(fd, e.bytes.as_ptr(), e.tamanho_da_familia()) } < 0 {
        return Err(fechar_com_erro(fd));
    }
    Ok(i64::from(fd))
}

// ---------------------------------------------------------------------------
// Opções, dados e datagramas (a interface que os natives usam).

/// `EINPROGRESS` (`OSError_inProgressErrorCode`).
#[cfg(unix)]
fn codigo_em_andamento() -> i64 {
    i64::from(so_rede::EINPROGRESS)
}

/// `SocketBase::IsBindError`.
#[cfg(unix)]
fn e_erro_de_bind(erro: i64) -> bool {
    [so_rede::EADDRINUSE, so_rede::EADDRNOTAVAIL, so_rede::EINVAL].contains(&(erro as i32))
}

/// `Socket_GetOption`: 0 `TCP_NODELAY`, 1 laço de multicast, 2 TTL de
/// multicast, 4 `SO_BROADCAST`; `None` com o erro no `errno`.
#[cfg(unix)]
fn ler_opcao(fd: i64, opcao: i64, v4: bool) -> Option<i64> {
    let fd = fd as i32;
    match opcao {
        0 => ler_opcao_int(fd, so_rede::IPPROTO_TCP, so_rede::TCP_NODELAY).map(i64::from),
        1 => {
            let (nivel, nome) = if v4 { (so_rede::IPPROTO_IP, so_rede::IP_MULTICAST_LOOP) } else { (so_rede::IPPROTO_IPV6, so_rede::IPV6_MULTICAST_LOOP) };
            ler_opcao_byte_ou_int(fd, nivel, nome)
        }
        2 => {
            let (nivel, nome) = if v4 { (so_rede::IPPROTO_IP, so_rede::IP_MULTICAST_TTL) } else { (so_rede::IPPROTO_IPV6, so_rede::IPV6_MULTICAST_HOPS) };
            ler_opcao_byte_ou_int(fd, nivel, nome)
        }
        4 => ler_opcao_int(fd, so_rede::SOL_SOCKET, so_rede::SO_BROADCAST).map(i64::from),
        _ => None,
    }
}

/// Opções de multicast IPv4 são um byte no macOS e um inteiro no Linux.
#[cfg(unix)]
fn ler_opcao_byte_ou_int(fd: i32, nivel: i32, nome: i32) -> Option<i64> {
    let mut v = [0u8; 4];
    let mut n = 4u32;
    // SAFETY: `v` tem 4 bytes; o sistema diz quantos usou.
    if unsafe { getsockopt(fd, nivel, nome, v.as_mut_ptr().cast(), &mut n) } != 0 {
        return None;
    }
    Some(if n == 1 { i64::from(v[0]) } else { i64::from(i32::from_ne_bytes(v)) })
}

/// `Socket_SetOption` (as mesmas opções de [`ler_opcao`]).
#[cfg(unix)]
fn definir_opcao(fd: i64, opcao: i64, v4: bool, valor: i64) -> bool {
    let fd = fd as i32;
    let (nivel, nome) = match opcao {
        0 => (so_rede::IPPROTO_TCP, so_rede::TCP_NODELAY),
        1 if v4 => (so_rede::IPPROTO_IP, so_rede::IP_MULTICAST_LOOP),
        1 => (so_rede::IPPROTO_IPV6, so_rede::IPV6_MULTICAST_LOOP),
        2 if v4 => (so_rede::IPPROTO_IP, so_rede::IP_MULTICAST_TTL),
        2 => (so_rede::IPPROTO_IPV6, so_rede::IPV6_MULTICAST_HOPS),
        4 => (so_rede::SOL_SOCKET, so_rede::SO_BROADCAST),
        _ => return false,
    };
    definir_opcao_int(fd, nivel, nome, valor as i32)
}

/// `setsockopt` com os bytes de `dados`.
#[cfg(unix)]
fn definir_opcao_bruta(fd: i64, nivel: i64, opcao: i64, dados: &[u8]) -> bool {
    // SAFETY: `dados` vive durante a chamada.
    unsafe { setsockopt(fd as i32, nivel as i32, opcao as i32, dados.as_ptr().cast(), dados.len() as u32) == 0 }
}

/// `getsockopt` nos bytes de `dados` (encurtados ao tamanho devolvido).
#[cfg(unix)]
fn ler_opcao_bruta(fd: i64, nivel: i64, opcao: i64, dados: &mut Vec<u8>) -> bool {
    let mut n = dados.len() as u32;
    // SAFETY: `dados` é gravável com `n` bytes.
    let ok = unsafe { getsockopt(fd as i32, nivel as i32, opcao as i32, dados.as_mut_ptr().cast(), &mut n) } == 0;
    dados.truncate(n as usize);
    ok
}

/// `RawSocketOption_GetOptionValue`: as constantes do sistema.
#[cfg(unix)]
fn constante_de_opcao_bruta(chave: i64) -> Option<i64> {
    Some(i64::from(match chave {
        0 => so_rede::SOL_SOCKET,
        1 => so_rede::IPPROTO_IP,
        2 => so_rede::IP_MULTICAST_IF,
        3 => so_rede::IPPROTO_IPV6,
        4 => so_rede::IPV6_MULTICAST_IF,
        5 => so_rede::IPPROTO_TCP,
        6 => so_rede::IPPROTO_UDP,
        _ => return None,
    }))
}

/// Os bytes prontos para leitura no soquete (-1 se o sistema não diz).
#[cfg(unix)]
fn disponivel(s: &SoqueteNativo) -> i64 {
    bytes_disponiveis(s.descritor())
}

/// Lê o que houver (0 sem dados agora).
#[cfg(unix)]
fn ler_de(s: &SoqueteNativo, destino: &mut [u8]) -> ResultadoIo<usize> {
    ler_do_soquete(s.descritor(), destino)
}

/// Escreve o que couber (0 com o soquete cheio).
#[cfg(unix)]
fn escrever_em(s: &SoqueteNativo, dados: &[u8]) -> ResultadoIo<usize> {
    escrever_no_soquete(s.descritor(), dados)
}

/// Se há um datagrama (espiando um byte).
#[cfg(unix)]
fn ha_datagrama(fd: i64) -> bool {
    let mut b = [0u8; 1];
    // SAFETY: `b` tem 1 byte.
    unsafe { recvfrom(fd as i32, b.as_mut_ptr(), 1, so_rede::MSG_PEEK, std::ptr::null_mut(), std::ptr::null_mut()) >= 0 }
}

/// Recebe um datagrama (`None` se não há nenhum agora).
#[cfg(unix)]
fn receber_datagrama(fd: i64, maximo: usize) -> ResultadoIo<Option<(Vec<u8>, EnderecoSo)>> {
    let mut buf = vec![0u8; maximo];
    let mut e = EnderecoSo::vazio();
    let mut n = 128u32;
    loop {
        // SAFETY: o buffer e o endereço são graváveis com os tamanhos dados.
        let r = unsafe { recvfrom(fd as i32, buf.as_mut_ptr(), buf.len(), 0, e.bytes.as_mut_ptr(), &mut n) };
        if r >= 0 {
            buf.truncate(r as usize);
            e.tamanho = n;
            return Ok(Some((buf, e)));
        }
        let err = std::io::Error::last_os_error();
        match err.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok(None),
            _ => return Err(ErroDoSo::de(&err)),
        }
    }
}

/// Envia um datagrama; 0 se o soquete está cheio.
#[cfg(unix)]
fn enviar_datagrama(fd: i64, dados: &[u8], e: &EnderecoSo) -> ResultadoIo<usize> {
    loop {
        // SAFETY: os dados e o endereço são válidos.
        let r = unsafe { sendto(fd as i32, dados.as_ptr(), dados.len(), 0, e.bytes.as_ptr(), e.tamanho_da_familia()) };
        if r >= 0 {
            return Ok(r as usize);
        }
        let err = std::io::Error::last_os_error();
        match err.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok(0),
            _ => return Err(ErroDoSo::de(&err)),
        }
    }
}

/// `SocketBase::HasPendingWrite`: só o Windows tem escritas em andamento.
#[cfg(unix)]
fn escrita_pendente(_s: &SoqueteNativo) -> bool {
    false
}

/// O tipo do descritor (0 terminal, 1 pipe, 2 arquivo, 4 outro).
#[cfg(unix)]
fn tipo_do_descritor(fd: i64) -> ResultadoIo<i64> {
    use std::os::unix::fs::FileTypeExt;
    let f = std::mem::ManuallyDrop::new(arquivo_do_descritor(fd));
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
fn apagar_caminho_unix(caminho: &[u8]) {
    if caminho.first() == Some(&0) {
        return;
    }
    if let Ok(c) = std::ffi::CString::new(caminho) {
        // SAFETY: caminho C válido.
        unsafe { unlink(c.as_ptr()) };
    }
}
