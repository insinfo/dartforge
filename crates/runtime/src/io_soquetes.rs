// Runtime nativo: os soquetes do `dart:io` da VM (`runtime/bin/socket.cc`,
// `socket_base*.cc`, `socket_linux.cc`, `socket_macos.cc`): TCP (cliente e
// servidor, com o registro de soquetes de escuta compartilhados), UDP,
// domínio Unix, endereços (`InternetAddress`) e a resolução de nomes do
// IOService (pedidos 32–34).
//
// Os natives devolvem o que os da VM devolvem (`true`, um `OSError`, uma
// `Uint8List`…) e lançam o que eles lançam; o descritor fica no
// [`SoqueteNativo`] do campo nativo (`io_eventos.rs`), não bloqueante, e o
// manipulador de eventos avisa quando há o que ler ou escrever.
//
// A camada do sistema (`criar_soquete`, `EnderecoSo`, as constantes) tem a
// versão Unix (Linux e macOS, com as diferenças de layout e de números);
// no Windows os soquetes ainda não são suportados e as operações devolvem
// o erro do sistema `ERROR_NOT_SUPPORTED`.

/// `SocketAddress::TYPE_*`.
const TIPO_IPV4: i64 = 0;
const TIPO_IPV6: i64 = 1;
const TIPO_UNIX: i64 = 2;

/// O erro dos soquetes onde ainda não há suporte (Windows).
#[cfg(windows)]
fn soquetes_nao_suportados() -> ErroDoSo {
    ErroDoSo::do_codigo(50 /* ERROR_NOT_SUPPORTED */)
}

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

/// Um `sockaddr_storage` e o tamanho usado.
#[derive(Clone)]
struct EnderecoSo {
    bytes: [u8; 128],
    tamanho: u32,
}

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

/// O endereço de um `Uint8List` Dart (4 ou 16 bytes).
fn endereco_de_lista(h: i64) -> Option<EnderecoSo> {
    #[cfg(unix)]
    {
        EnderecoSo::de_ip(&bytes_da_lista_tipada(h)?)
    }
    #[cfg(windows)]
    {
        let _ = h;
        None
    }
}

/// A `ArgumentError` de um endereço mal formado (a VM propaga um erro da
/// API: "Unexpected type for socket address").
fn lancar_endereco_invalido() {
    let msg = alocar_str("Unexpected type for socket address");
    let e = com_raizes(&[msg], || dartforge_argument_error_new(msg, 0));
    com_raizes(&[e], || dartforge_exception_throw(e, 3));
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

// ---------------------------------------------------------------------------
// Os natives de `_NativeSocket`.

/// A resposta de um native de criação: `true` com o soquete no objeto, ou
/// o `OSError`.
fn soquete_criado(objeto: i64, r: ResultadoIo<i64>, finalizador: FinalizadorDeSoquete) -> i64 {
    match r {
        Ok(fd) => {
            definir_soquete_no_objeto(objeto, fd, finalizador);
            dart_bool(true)
        }
        Err(e) => e.para_dart(),
    }
}

/// O endereço de conexão: IP de `addr`, `porta` e o escopo IPv6.
fn endereco_de_destino(addr: i64, porta: i64, escopo: i64) -> Option<EnderecoSo> {
    #[cfg(unix)]
    {
        let mut e = endereco_de_lista(addr)?;
        e.gravar_porta(porta);
        e.gravar_escopo(escopo);
        Some(e)
    }
    #[cfg(windows)]
    {
        let _ = (addr, porta, escopo);
        None
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateConnect(this: i64, addr: i64, porta: i64, escopo: i64) -> i64 {
    #[cfg(unix)]
    {
        let Some(e) = endereco_de_destino(addr, porta, escopo) else {
            lancar_endereco_invalido();
            return 0;
        };
        soquete_criado(this, criar_conexao(&e), FinalizadorDeSoquete::Normal)
    }
    #[cfg(windows)]
    {
        let _ = (this, addr, porta, escopo);
        soquetes_nao_suportados().para_dart()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateBindConnect(this: i64, addr: i64, porta: i64, origem: i64, porta_origem: i64, escopo: i64) -> i64 {
    #[cfg(unix)]
    {
        let (Some(e), Some(o)) = (endereco_de_destino(addr, porta, escopo), endereco_de_destino(origem, porta_origem, 0)) else {
            lancar_endereco_invalido();
            return 0;
        };
        soquete_criado(this, criar_conexao_com_origem(&e, &o), FinalizadorDeSoquete::Normal)
    }
    #[cfg(windows)]
    {
        let _ = (this, addr, porta, origem, porta_origem, escopo);
        soquetes_nao_suportados().para_dart()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateUnixDomainConnect(this: i64, caminho: i64, _ns: i64) -> i64 {
    #[cfg(unix)]
    {
        let r = EnderecoSo::de_caminho_unix(&utf8_de_texto(caminho)).and_then(|e| criar_conexao_unix(&e, None));
        soquete_criado(this, r, FinalizadorDeSoquete::Normal)
    }
    #[cfg(windows)]
    {
        let _ = (this, caminho);
        soquetes_nao_suportados().para_dart()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateUnixDomainBindConnect(this: i64, caminho: i64, origem: i64, _ns: i64) -> i64 {
    #[cfg(unix)]
    {
        let r = EnderecoSo::de_caminho_unix(&utf8_de_texto(caminho)).and_then(|e| {
            let o = EnderecoSo::de_caminho_unix(&utf8_de_texto(origem))?;
            criar_conexao_unix(&e, Some(&o))
        });
        soquete_criado(this, r, FinalizadorDeSoquete::Normal)
    }
    #[cfg(windows)]
    {
        let _ = (this, caminho, origem);
        soquetes_nao_suportados().para_dart()
    }
}

/// `ServerSocket_CreateBindListen`: o soquete de escuta, compartilhado com
/// outro do mesmo (endereço, porta) quando os dois pedem `shared`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ServerSocket_CreateBindListen(
    this: i64,
    addr: i64,
    porta: i64,
    fila: i64,
    so_v6: u8,
    compartilhado: u8,
    escopo: i64,
) -> i64 {
    #[cfg(unix)]
    {
        let Some(e) = endereco_de_destino(addr, porta, escopo) else {
            lancar_endereco_invalido();
            return 0;
        };
        criar_escuta_registrada(this, e, fila, so_v6 != 0, compartilhado != 0)
    }
    #[cfg(windows)]
    {
        let _ = (this, addr, porta, fila, so_v6, compartilhado, escopo);
        soquetes_nao_suportados().para_dart()
    }
}

/// `ListeningSocketRegistry::CreateBindListen`.
#[cfg(unix)]
fn criar_escuta_registrada(this: i64, e: EnderecoSo, fila: i64, so_v6: bool, compartilhado: bool) -> i64 {
    let mut reg = registro_de_escuta();
    let porta = e.porta();
    if porta > 0 {
        if let Some(existente) = reg.soquetes.iter_mut().find(|s| s.porta == porta && s.endereco == e.ip()) {
            if !existente.compartilhado || !compartilhado {
                drop(reg);
                return ErroDoSo {
                    codigo: -1,
                    mensagem: "The shared flag to bind() needs to be `true` if binding multiple times on the same (address, port) combination.".to_string(),
                }
                .para_dart();
            }
            if existente.so_v6 != so_v6 {
                drop(reg);
                return ErroDoSo {
                    codigo: -1,
                    mensagem: "The v6Only flag to bind() needs to be the same if binding multiple times on the same (address, port) combination.".to_string(),
                }
                .para_dart();
            }
            existente.usos += 1;
            let fd = existente.descritor;
            let p = SoqueteNativo::novo(fd);
            reg.por_soquete.insert(p, fd);
            drop(reg);
            reusar_soquete_no_objeto(this, p, FinalizadorDeSoquete::Escuta);
            return dart_bool(true);
        }
    }
    let fd = match criar_escuta(&e, fila, so_v6) {
        Ok(fd) => fd,
        Err(erro) => {
            drop(reg);
            return erro.para_dart();
        }
    };
    let porta_alocada = porta_local(fd);
    let p = SoqueteNativo::novo(fd);
    reg.soquetes.push(SoqueteDeEscuta { endereco: e.ip(), porta: porta_alocada, so_v6, compartilhado, usos: 1, descritor: fd, caminho_unix: None });
    reg.por_soquete.insert(p, fd);
    drop(reg);
    reusar_soquete_no_objeto(this, p, FinalizadorDeSoquete::Escuta);
    dart_bool(true)
}

/// `ServerSocket_CreateUnixDomainBindListen`: como o de TCP, com o
/// compartilhamento pelo caminho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ServerSocket_CreateUnixDomainBindListen(this: i64, caminho: i64, fila: i64, compartilhado: u8, _ns: i64) -> i64 {
    #[cfg(unix)]
    {
        let caminho = utf8_de_texto(caminho);
        let compartilhado = compartilhado != 0;
        let mut reg = registro_de_escuta();
        if let Some(existente) = reg.soquetes.iter_mut().find(|s| s.caminho_unix.as_deref() == Some(&caminho[..])) {
            if !existente.compartilhado || !compartilhado {
                drop(reg);
                return ErroDoSo {
                    codigo: -1,
                    mensagem: "The shared flag to bind() needs to be `true` if binding multiple times on the same path.".to_string(),
                }
                .para_dart();
            }
            existente.usos += 1;
            let fd = existente.descritor;
            let p = SoqueteNativo::novo(fd);
            reg.por_soquete.insert(p, fd);
            drop(reg);
            reusar_soquete_no_objeto(this, p, FinalizadorDeSoquete::Escuta);
            return dart_bool(true);
        }
        let fd = match EnderecoSo::de_caminho_unix(&caminho).and_then(|e| criar_escuta_unix(&e, fila)) {
            Ok(fd) => fd,
            Err(e) => {
                drop(reg);
                return e.para_dart();
            }
        };
        let p = SoqueteNativo::novo(fd);
        reg.soquetes.push(SoqueteDeEscuta {
            endereco: Vec::new(),
            porta: 0,
            so_v6: false,
            compartilhado,
            usos: 1,
            descritor: fd,
            caminho_unix: Some(caminho),
        });
        reg.por_soquete.insert(p, fd);
        drop(reg);
        reusar_soquete_no_objeto(this, p, FinalizadorDeSoquete::Escuta);
        dart_bool(true)
    }
    #[cfg(windows)]
    {
        let _ = (this, caminho, fila);
        ErroDoSo { codigo: -1, mensagem: "Unix domain sockets are not available on this operating system.".to_string() }.para_dart()
    }
}

/// `ServerSocket_Accept(socket)`: `true` com a conexão nova no objeto.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ServerSocket_Accept(this: i64, novo: i64) -> u8 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    #[cfg(unix)]
    {
        let fd = aceitar(s.descritor());
        if fd >= 0 {
            definir_soquete_no_objeto(novo, fd, FinalizadorDeSoquete::Normal);
            return 1;
        }
        0
    }
    #[cfg(windows)]
    {
        let _ = (s, novo);
        0
    }
}

/// `Socket_Available`: os bytes prontos (1 se o sistema não sabe dizer).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_Available(this: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    #[cfg(unix)]
    {
        let n = bytes_disponiveis(s.descritor());
        if n >= 0 { n } else { 1 }
    }
    #[cfg(windows)]
    {
        let _ = s;
        1
    }
}

/// `Socket_Read(len)`: os bytes lidos, `null` se nada veio; lança o
/// `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_Read(this: i64, n: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    if n < 0 {
        lancar_os_error(&ErroDoSo::argumento_invalido());
        return 0;
    }
    let mut buf = vec![0u8; n as usize];
    #[cfg(unix)]
    let r = ler_do_soquete(s.descritor(), &mut buf);
    #[cfg(windows)]
    let r: ResultadoIo<usize> = {
        let _ = s;
        Err(soquetes_nao_suportados())
    };
    match r {
        Ok(0) => 0,
        Ok(lidos) => {
            buf.truncate(lidos);
            dart_bytes(buf)
        }
        Err(e) => {
            lancar_os_error(&e);
            0
        }
    }
}

/// Lança o `OSError` (os natives de soquete lançam em vez de devolver).
fn lancar_os_error(e: &ErroDoSo) {
    let erro = e.para_dart();
    if dartforge_exception_pending() == 0 {
        com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
    }
}

/// `Socket_WriteList(buffer, offset, bytes)`: os bytes escritos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_WriteList(this: i64, buffer: i64, inicio: i64, n: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    let bytes = bytes_da_lista_tipada(buffer).unwrap_or_default();
    let inicio = (inicio.max(0) as usize).min(bytes.len());
    let fim = (inicio + n.max(0) as usize).min(bytes.len());
    #[cfg(unix)]
    let r = escrever_no_soquete(s.descritor(), &bytes[inicio..fim]);
    #[cfg(windows)]
    let r: ResultadoIo<usize> = {
        let _ = (s, fim);
        Err(soquetes_nao_suportados())
    };
    match r {
        Ok(escritos) => escritos as i64,
        Err(e) => {
            lancar_os_error(&e);
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_HasPendingWrite(_this: i64) -> u8 {
    0
}

/// `Socket_GetPort`: a porta local, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetPort(this: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    #[cfg(unix)]
    {
        let p = porta_local(s.descritor());
        if p > 0 {
            return dart_int(p);
        }
        ultimo_erro().para_dart()
    }
    #[cfg(windows)]
    {
        let _ = s;
        soquetes_nao_suportados().para_dart()
    }
}

/// `Socket_GetRemotePeer`: `[[tipo, texto, bytes], porta]` (`[tipo,
/// texto]` no domínio Unix).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetRemotePeer(this: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    #[cfg(unix)]
    match par_remoto(s.descritor()) {
        Ok((e, porta)) => {
            let tipo = e.tipo();
            let texto = alocar_str(&e.texto());
            let entrada = com_raizes(&[texto], || {
                if tipo == TIPO_UNIX {
                    dart_lista_fixa(&[dart_int(tipo), texto])
                } else {
                    let ip = dart_bytes(e.ip());
                    com_raizes(&[ip], || dart_lista_fixa(&[dart_int(tipo), texto, ip]))
                }
            });
            com_raizes(&[entrada], || dart_lista_fixa(&[entrada, dart_int(porta)]))
        }
        Err(e) => {
            lancar_os_error(&e);
            0
        }
    }
    #[cfg(windows)]
    {
        let _ = s;
        lancar_os_error(&soquetes_nao_suportados());
        0
    }
}

/// `Socket_GetError`: o `SO_ERROR` como `OSError`, ou `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetError(this: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    #[cfg(unix)]
    {
        let c = erro_pendente(s.descritor());
        if c != 0 { ErroDoSo::do_codigo(c).para_dart() } else { 0 }
    }
    #[cfg(windows)]
    {
        let _ = s;
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetFD(this: i64) -> i64 {
    soquete_do_objeto(this).map_or(0, SoqueteNativo::descritor)
}

/// `_getSocketType(nativeSocket)`: o tipo do descritor (terminal, pipe,
/// arquivo, outro) ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetType(soquete: i64) -> i64 {
    let Some(s) = soquete_do_objeto(soquete) else { return 0 };
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        let f = std::mem::ManuallyDrop::new(arquivo_do_descritor(s.descritor()));
        match f.metadata() {
            Ok(m) => {
                let t = m.file_type();
                dart_int(if t.is_char_device() {
                    0
                } else if t.is_fifo() {
                    1
                } else if t.is_file() {
                    2
                } else {
                    4
                })
            }
            Err(e) => ErroDoSo::de(&e).para_dart(),
        }
    }
    #[cfg(windows)]
    {
        let _ = s;
        soquetes_nao_suportados().para_dart()
    }
}

/// `Socket_GetStdioHandle(socket, num)`: a entrada/saída padrão `num` como
/// soquete (a entrada assíncrona de `stdin`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetStdioHandle(soquete: i64, num: i64) -> u8 {
    let fd = descritor_padrao(num.clamp(0, 2));
    definir_soquete_no_objeto(soquete, fd, FinalizadorDeSoquete::Stdio);
    u8::from(fd >= 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetSocketId(this: i64) -> i64 {
    let p = campo_nativo(this);
    if p == 0 {
        lancar_erro_interno("No native peer");
    }
    p
}

/// `Socket_SetSocketId(id, typeFlags)`: o objeto passa a usar o descritor
/// `id` (o `_NativeSocket` criado para um descritor já aberto).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SetSocketId(this: i64, id: i64, bandeiras: i64) {
    let f = if bandeiras & (1 << 21) != 0 { FinalizadorDeSoquete::Sinal } else { FinalizadorDeSoquete::Normal };
    definir_soquete_no_objeto(this, id, f);
}

/// `SocketBase_IsBindError(errno)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SocketBase_IsBindError(_this: i64, erro: i64) -> u8 {
    #[cfg(unix)]
    {
        u8::from([so_rede::EADDRINUSE, so_rede::EADDRNOTAVAIL, so_rede::EINVAL].contains(&(erro as i32)))
    }
    #[cfg(windows)]
    {
        // WSAEADDRINUSE, WSAEADDRNOTAVAIL, WSAEINVAL.
        u8::from([10048, 10049, 10022].contains(&erro))
    }
}

/// `OSError_inProgressErrorCode`: o `EINPROGRESS` do sistema.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_OSError_inProgressErrorCode() -> i64 {
    #[cfg(unix)]
    {
        i64::from(so_rede::EINPROGRESS)
    }
    #[cfg(windows)]
    {
        10036 // WSAEINPROGRESS
    }
}

/// `Socket_GetOption(option, protocol)`: `TCP_NODELAY`, laço e TTL de
/// multicast, `SO_BROADCAST`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetOption(this: i64, opcao: i64, protocolo: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    #[cfg(unix)]
    {
        let fd = s.descritor() as i32;
        let v4 = protocolo == TIPO_IPV4;
        let r = match opcao {
            0 => ler_opcao_int(fd, so_rede::IPPROTO_TCP, so_rede::TCP_NODELAY).map(|v| dart_bool(v != 0)),
            1 => {
                let (nivel, nome) = if v4 { (so_rede::IPPROTO_IP, so_rede::IP_MULTICAST_LOOP) } else { (so_rede::IPPROTO_IPV6, so_rede::IPV6_MULTICAST_LOOP) };
                ler_opcao_byte_ou_int(fd, nivel, nome).map(|v| dart_bool(v != 0))
            }
            2 => {
                let (nivel, nome) = if v4 { (so_rede::IPPROTO_IP, so_rede::IP_MULTICAST_TTL) } else { (so_rede::IPPROTO_IPV6, so_rede::IPV6_MULTICAST_HOPS) };
                ler_opcao_byte_ou_int(fd, nivel, nome).map(dart_int)
            }
            4 => ler_opcao_int(fd, so_rede::SOL_SOCKET, so_rede::SO_BROADCAST).map(|v| dart_bool(v != 0)),
            _ => None,
        };
        match r {
            Some(v) => v,
            None => {
                lancar_os_error(&ultimo_erro());
                0
            }
        }
    }
    #[cfg(windows)]
    {
        let _ = (s, opcao, protocolo);
        lancar_os_error(&soquetes_nao_suportados());
        0
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

/// `Socket_SetOption(option, protocol, value)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SetOption(this: i64, opcao: i64, protocolo: i64, valor: i64) {
    let Some(s) = soquete_do_objeto(this) else { return };
    #[cfg(unix)]
    {
        let fd = s.descritor() as i32;
        let v4 = protocolo == TIPO_IPV4;
        let booleano = || HEAP.with(|h| matches!(h.borrow().try_get(valor), Some(Value::BoxedBool(true))));
        let inteiro = || HEAP.with(|h| h.borrow().int_de_ref(valor)).unwrap_or(0) as i32;
        let ok = match opcao {
            0 => definir_opcao_int(fd, so_rede::IPPROTO_TCP, so_rede::TCP_NODELAY, i32::from(booleano())),
            1 => {
                let (nivel, nome) = if v4 { (so_rede::IPPROTO_IP, so_rede::IP_MULTICAST_LOOP) } else { (so_rede::IPPROTO_IPV6, so_rede::IPV6_MULTICAST_LOOP) };
                definir_opcao_int(fd, nivel, nome, i32::from(booleano()))
            }
            2 => {
                let (nivel, nome) = if v4 { (so_rede::IPPROTO_IP, so_rede::IP_MULTICAST_TTL) } else { (so_rede::IPPROTO_IPV6, so_rede::IPV6_MULTICAST_HOPS) };
                definir_opcao_int(fd, nivel, nome, inteiro())
            }
            4 => definir_opcao_int(fd, so_rede::SOL_SOCKET, so_rede::SO_BROADCAST, i32::from(booleano())),
            _ => {
                let msg = alocar_str("option to setOption() is outside expected range");
                let e = com_raizes(&[msg], || dartforge_argument_error_new(msg, 0));
                com_raizes(&[e], || dartforge_exception_throw(e, 3));
                return;
            }
        };
        if !ok {
            lancar_os_error(&ultimo_erro());
        }
    }
    #[cfg(windows)]
    {
        let _ = (s, opcao, protocolo, valor);
        lancar_os_error(&soquetes_nao_suportados());
    }
}

/// `Socket_SetRawOption(level, option, data)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SetRawOption(this: i64, nivel: i64, opcao: i64, dados: i64) {
    let Some(s) = soquete_do_objeto(this) else { return };
    #[cfg(unix)]
    {
        let b = bytes_da_lista_tipada(dados).unwrap_or_default();
        // SAFETY: `b` vive durante a chamada.
        if unsafe { setsockopt(s.descritor() as i32, nivel as i32, opcao as i32, b.as_ptr().cast(), b.len() as u32) } != 0 {
            lancar_os_error(&ultimo_erro());
        }
    }
    #[cfg(windows)]
    {
        let _ = (s, nivel, opcao, dados);
        lancar_os_error(&soquetes_nao_suportados());
    }
}

/// `Socket_GetRawOption(level, option, data)`: grava o valor em `data`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetRawOption(this: i64, nivel: i64, opcao: i64, dados: i64) {
    let Some(s) = soquete_do_objeto(this) else { return };
    #[cfg(unix)]
    {
        let mut b = bytes_da_lista_tipada(dados).unwrap_or_default();
        let mut n = b.len() as u32;
        // SAFETY: `b` é gravável com `n` bytes.
        if unsafe { getsockopt(s.descritor() as i32, nivel as i32, opcao as i32, b.as_mut_ptr().cast(), &mut n) } != 0 {
            lancar_os_error(&ultimo_erro());
            return;
        }
        b.truncate(n as usize);
        gravar_bytes_na_lista(dados, 0, &b);
    }
    #[cfg(windows)]
    {
        let _ = (s, nivel, opcao, dados);
        lancar_os_error(&soquetes_nao_suportados());
    }
}

/// `RawSocketOption_GetOptionValue(key)`: as constantes do sistema.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_RawSocketOption_GetOptionValue(chave: i64) -> i64 {
    #[cfg(unix)]
    {
        i64::from(match chave {
            0 => so_rede::SOL_SOCKET,
            1 => so_rede::IPPROTO_IP,
            2 => so_rede::IP_MULTICAST_IF,
            3 => so_rede::IPPROTO_IPV6,
            4 => so_rede::IPV6_MULTICAST_IF,
            5 => so_rede::IPPROTO_TCP,
            6 => so_rede::IPPROTO_UDP,
            _ => {
                let msg = alocar_str("option to getOptionValue() is outside expected range");
                let e = com_raizes(&[msg], || dartforge_argument_error_new(msg, 0));
                com_raizes(&[e], || dartforge_exception_throw(e, 3));
                return 0;
            }
        })
    }
    #[cfg(windows)]
    {
        // SOL_SOCKET, IPPROTO_IP, IP_MULTICAST_IF, IPPROTO_IPV6,
        // IPV6_MULTICAST_IF, IPPROTO_TCP, IPPROTO_UDP.
        [0xffff, 0, 9, 41, 9, 6, 17].get(chave as usize).copied().unwrap_or(0)
    }
}

/// `Socket_Fatal(msg)`: erro irrecuperável do `dart:io` (a VM aborta).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_Fatal(msg: i64) {
    let texto = if msg == 0 { String::from("(null)") } else { HEAP.with(|h| h.borrow().texto(msg).para_string()) };
    eprintln!("Fatal error in dart:io (socket): {texto}");
    std::process::abort();
}

// ---------------------------------------------------------------------------
// UDP.

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

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateBindDatagram(this: i64, addr: i64, porta: i64, reusar_endereco: u8, reusar_porta: u8, ttl: i64) -> i64 {
    #[cfg(unix)]
    {
        let Some(e) = endereco_de_destino(addr, porta, 0) else {
            lancar_endereco_invalido();
            return 0;
        };
        soquete_criado(this, criar_datagrama(&e, reusar_endereco != 0, reusar_porta != 0, ttl), FinalizadorDeSoquete::Normal)
    }
    #[cfg(windows)]
    {
        let _ = (this, addr, porta, reusar_endereco, reusar_porta, ttl);
        soquetes_nao_suportados().para_dart()
    }
}

/// `Socket_AvailableDatagram`: se há um datagrama (espiando um byte).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_AvailableDatagram(this: i64) -> u8 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    #[cfg(unix)]
    {
        let mut b = [0u8; 1];
        // SAFETY: `b` tem 1 byte.
        u8::from(unsafe { recvfrom(s.descritor() as i32, b.as_mut_ptr(), 1, so_rede::MSG_PEEK, std::ptr::null_mut(), std::ptr::null_mut()) } >= 0)
    }
    #[cfg(windows)]
    {
        let _ = s;
        0
    }
}

/// O tamanho máximo de um datagrama UDP (o buffer de recepção da VM).
const TAMANHO_MAXIMO_DE_DATAGRAMA: usize = 65536;

/// `Socket_RecvFrom`: o `Datagram` recebido (pelo `_makeDatagram` do
/// patch), ou `null` se não havia nenhum.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_RecvFrom(this: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    #[cfg(unix)]
    {
        let mut buf = vec![0u8; TAMANHO_MAXIMO_DE_DATAGRAMA];
        let mut e = EnderecoSo::vazio();
        let mut n = 128u32;
        let lidos = loop {
            // SAFETY: o buffer e o endereço são graváveis com os tamanhos dados.
            let r = unsafe { recvfrom(s.descritor() as i32, buf.as_mut_ptr(), buf.len(), 0, e.bytes.as_mut_ptr(), &mut n) };
            if r >= 0 {
                break r as usize;
            }
            let err = std::io::Error::last_os_error();
            match err.kind() {
                std::io::ErrorKind::Interrupted => continue,
                std::io::ErrorKind::WouldBlock => return 0,
                _ => {
                    lancar_os_error(&ErroDoSo::de(&err));
                    return 0;
                }
            }
        };
        e.tamanho = n;
        buf.truncate(lidos);
        let Some(f) = ajudante("_dartforgeDatagrama") else {
            panic!("bug do compilador: dart:io sem `_dartforgeDatagrama` registrado");
        };
        // SAFETY: registrada pela sobreposição de `dart:io` com a
        // assinatura (`Uint8List`, `String`, `Uint8List`, `int`, `int`).
        let g: extern "C" fn(i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
        let dados = dart_bytes(buf);
        let texto = com_raizes(&[dados], || alocar_str(&e.texto()));
        let ip = com_raizes(&[dados, texto], || dart_bytes(e.ip()));
        com_raizes(&[dados, texto, ip], || g(dados, texto, ip, e.porta(), e.tipo()))
    }
    #[cfg(windows)]
    {
        let _ = s;
        0
    }
}

/// `Socket_SendTo(buffer, offset, bytes, address, port)`: os bytes
/// enviados (0 se o soquete está cheio).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SendTo(this: i64, buffer: i64, inicio: i64, n: i64, addr: i64, porta: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    #[cfg(unix)]
    {
        let Some(e) = endereco_de_destino(addr, porta, 0) else {
            lancar_endereco_invalido();
            return 0;
        };
        let bytes = bytes_da_lista_tipada(buffer).unwrap_or_default();
        let inicio = (inicio.max(0) as usize).min(bytes.len());
        let fim = (inicio + n.max(0) as usize).min(bytes.len());
        loop {
            // SAFETY: os dados e o endereço são válidos.
            let r = unsafe { sendto(s.descritor() as i32, bytes[inicio..].as_ptr(), fim - inicio, 0, e.bytes.as_ptr(), e.tamanho_da_familia()) };
            if r >= 0 {
                return r as i64;
            }
            let err = std::io::Error::last_os_error();
            match err.kind() {
                std::io::ErrorKind::Interrupted => continue,
                std::io::ErrorKind::WouldBlock => return 0,
                _ => {
                    lancar_os_error(&ErroDoSo::de(&err));
                    return 0;
                }
            }
        }
    }
    #[cfg(windows)]
    {
        let _ = (s, buffer, inicio, n, addr, porta);
        lancar_os_error(&soquetes_nao_suportados());
        0
    }
}

// ---------------------------------------------------------------------------
// `InternetAddress`.

/// `InternetAddress_Parse(address)`: os bytes do endereço numérico, ou
/// `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_InternetAddress_Parse(texto: i64) -> i64 {
    let t = HEAP.with(|h| h.borrow().texto(texto).para_string());
    #[cfg(unix)]
    {
        interpretar_endereco(&t).map_or(0, dart_bytes)
    }
    #[cfg(windows)]
    {
        match t.parse::<std::net::IpAddr>() {
            Ok(std::net::IpAddr::V4(v)) => dart_bytes(v.octets().to_vec()),
            Ok(std::net::IpAddr::V6(v)) => dart_bytes(v.octets().to_vec()),
            Err(_) => 0,
        }
    }
}

/// `InternetAddress_RawAddrToString(address)`: o texto numérico.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_InternetAddress_RawAddrToString(addr: i64) -> i64 {
    let bytes = bytes_da_lista_tipada(addr).unwrap_or_default();
    #[cfg(unix)]
    {
        let Some(e) = EnderecoSo::de_ip(&bytes) else {
            lancar_endereco_invalido();
            return 0;
        };
        alocar_str(&e.texto())
    }
    #[cfg(windows)]
    {
        let t = match bytes.len() {
            4 => std::net::Ipv4Addr::from(<[u8; 4]>::try_from(&bytes[..]).expect("4 bytes")).to_string(),
            16 => std::net::Ipv6Addr::from(<[u8; 16]>::try_from(&bytes[..]).expect("16 bytes")).to_string(),
            _ => String::new(),
        };
        alocar_str(&t)
    }
}

/// `InternetAddress_ParseScopedLinkLocalAddress(address)`: o escopo de um
/// IPv6 de enlace local com `%interface`, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_InternetAddress_ParseScopedLinkLocalAddress(texto: i64) -> i64 {
    let t = HEAP.with(|h| h.borrow().texto(texto).para_string());
    #[cfg(unix)]
    {
        match resolver_nome(&t, TIPO_IPV6) {
            Ok(v) if !v.is_empty() => dart_int(v[0].escopo()),
            Ok(_) => ErroDoSo::do_codigo(so_rede::EINVAL).para_dart(),
            Err(e) => e.para_dart(),
        }
    }
    #[cfg(windows)]
    {
        let _ = t;
        soquetes_nao_suportados().para_dart()
    }
}

// ---------------------------------------------------------------------------
// Os pedidos de rede do IOService (32 lookup, 33 interfaces, 34 reverso).

/// `Socket::LookupRequest(host, type)`: `[0, [tipo, texto, bytes, escopo]…]`.
fn pedido_de_resolucao(d: &[Portavel]) -> Portavel {
    let (Some(host), Some(tipo)) = (d.first().and_then(Portavel::str), d.get(1).and_then(Portavel::int)) else {
        return argumento_invalido();
    };
    #[cfg(unix)]
    {
        resposta(resolver_nome(host, tipo), |v| {
            let mut l = vec![Portavel::Int(0)];
            for e in v {
                l.push(Portavel::Lista(vec![Portavel::Int(e.tipo()), Portavel::Str(e.texto()), Portavel::Bytes(e.ip()), Portavel::Int(e.escopo())]));
            }
            Portavel::Lista(l)
        })
    }
    #[cfg(windows)]
    {
        let _ = (host, tipo);
        soquetes_nao_suportados().para_resposta()
    }
}

/// `Socket::ListInterfacesRequest(type)`: `[0, [tipo, texto, bytes, nome,
/// índice]…]`.
fn pedido_de_interfaces(d: &[Portavel]) -> Portavel {
    let Some(tipo) = d.first().and_then(Portavel::int) else {
        return argumento_invalido();
    };
    #[cfg(unix)]
    {
        resposta(listar_interfaces(tipo), |v| {
            let mut l = vec![Portavel::Int(0)];
            for (e, nome, indice) in v {
                l.push(Portavel::Lista(vec![Portavel::Int(e.tipo()), Portavel::Str(e.texto()), Portavel::Bytes(e.ip()), Portavel::Str(nome), Portavel::Int(indice)]));
            }
            Portavel::Lista(l)
        })
    }
    #[cfg(windows)]
    {
        let _ = tipo;
        soquetes_nao_suportados().para_resposta()
    }
}

/// `Socket::ReverseLookupRequest(address)`: o nome do endereço.
fn pedido_de_resolucao_reversa(d: &[Portavel]) -> Portavel {
    let Some(ip) = d.first().and_then(Portavel::bytes) else {
        return argumento_invalido();
    };
    #[cfg(unix)]
    {
        let Some(e) = EnderecoSo::de_ip(ip) else {
            return argumento_invalido();
        };
        resposta(nome_do_endereco(&e), Portavel::Str)
    }
    #[cfg(windows)]
    {
        let _ = ip;
        soquetes_nao_suportados().para_resposta()
    }
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
