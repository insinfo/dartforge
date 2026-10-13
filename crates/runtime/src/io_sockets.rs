// Runtime nativo: os soquetes do `dart:io` da VM (`runtime/bin/socket.cc`):
// TCP (cliente e servidor, com o registro de soquetes de escuta
// compartilhados), UDP, domínio Unix, endereços (`InternetAddress`) e a
// resolução de nomes do IOService (pedidos 32–34).
//
// Os natives devolvem o que os da VM devolvem (`true`, um `OSError`, uma
// `Uint8List`…) e lançam o que eles lançam; o descritor fica no
// [`NativeSocket`] do campo nativo (`io_eventos.rs`), não bloqueante, e o
// manipulador de eventos avisa quando há o que ler ou escrever.
//
// Os natives não dependem do sistema: a camada do sistema (`OsAddress`,
// `create_connection`, `read_from`, `resolve_name`…) tem os mesmos nomes em
// `io_soquetes_unix.rs` (Linux e macOS) e em `io_windows_soquetes.rs`
// (Winsock, sobre a porta de conclusão de `io_windows_eventos.rs`).

/// `SocketAddress::TYPE_*`.
const TYPE_IPV4: i64 = 0;
const TYPE_IPV6: i64 = 1;
const TYPE_UNIX: i64 = 2;

/// Um `sockaddr_storage` e o tamanho usado.
#[derive(Clone)]
struct OsAddress {
    bytes: [u8; 128],
    size: u32,
}

/// O endereço de um `Uint8List` Dart (4 ou 16 bytes).
fn address_from_list(h: i64) -> Option<OsAddress> {
    OsAddress::from_ip(&typed_list_bytes(h)?)
}

/// A `ArgumentError` de um endereço mal formado (a VM propaga um erro da
/// API: "Unexpected type for socket address").
fn throw_invalid_address() {
    let msg = allocate_str("Unexpected type for socket address");
    let e = with_roots(&[msg], || dartforge_argument_error_new(msg, 0));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

/// Lança o `OSError` (os natives de soquete lançam em vez de devolver).
fn throw_os_error(e: &OsError) {
    let error = e.to_dart();
    if dartforge_exception_pending() == 0 {
        with_roots(&[error], || dartforge_exception_throw(error, 3));
    }
}

/// Lança o `ArgumentError(message)` (os erros da API da VM).
fn throw_argument_error(message: &str) {
    let msg = allocate_str(message);
    let e = with_roots(&[msg], || dartforge_argument_error_new(msg, 0));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

// ---------------------------------------------------------------------------
// Os natives de `_NativeSocket`.

/// A resposta de um native de criação: `true` com o soquete no objeto, ou
/// o `OSError`.
fn created_socket(object: i64, r: IoResult<i64>, finalizer: SocketFinalizer) -> i64 {
    match r {
        Ok(fd) => {
            set_socket_on_object(object, fd, finalizer);
            dart_bool(true)
        }
        Err(e) => e.to_dart(),
    }
}

/// O endereço de conexão: IP de `addr`, `port` e o escopo IPv6.
fn dest_address(addr: i64, port: i64, scope: i64) -> Option<OsAddress> {
    let mut e = address_from_list(addr)?;
    e.write_port(port);
    e.write_scope(scope);
    Some(e)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateConnect(this: i64, addr: i64, port: i64, scope: i64) -> i64 {
    let Some(e) = dest_address(addr, port, scope) else {
        throw_invalid_address();
        return 0;
    };
    created_socket(this, create_connection(&e), SocketFinalizer::Normal)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateBindConnect(this: i64, addr: i64, port: i64, origin: i64, source_port: i64, scope: i64) -> i64 {
    let (Some(e), Some(o)) = (dest_address(addr, port, scope), dest_address(origin, source_port, 0)) else {
        throw_invalid_address();
        return 0;
    };
    created_socket(this, create_connection_with_source(&e, &o), SocketFinalizer::Normal)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateUnixDomainConnect(this: i64, file_path: i64, _ns: i64) -> i64 {
    let r = OsAddress::from_unix_path(&utf8_of_string(file_path)).and_then(|e| create_unix_connection(&e, None));
    created_socket(this, r, SocketFinalizer::Normal)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateUnixDomainBindConnect(this: i64, file_path: i64, origin: i64, _ns: i64) -> i64 {
    let r = OsAddress::from_unix_path(&utf8_of_string(file_path)).and_then(|e| {
        let o = OsAddress::from_unix_path(&utf8_of_string(origin))?;
        create_unix_connection(&e, Some(&o))
    });
    created_socket(this, r, SocketFinalizer::Normal)
}

/// `ServerSocket_CreateBindListen`: o soquete de escuta, compartilhado com
/// outro do mesmo (endereço, porta) quando os dois pedem `shared`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ServerSocket_CreateBindListen(
    this: i64,
    addr: i64,
    port: i64,
    queue: i64,
    v6_only: u8,
    shared: u8,
    scope: i64,
) -> i64 {
    let Some(e) = dest_address(addr, port, scope) else {
        throw_invalid_address();
        return 0;
    };
    let mut reg = listen_record();
    let port = e.port();
    let (v6_only, shared) = (v6_only != 0, shared != 0);
    if port > 0 {
        if let Some(existing) = reg.sockets.iter_mut().find(|s| s.port == port && s.address == e.ip()) {
            let message = if !existing.shared || !shared {
                Some("The shared flag to bind() needs to be `true` if binding multiple times on the same (address, port) combination.")
            } else if existing.v6_only != v6_only {
                Some("The v6Only flag to bind() needs to be the same if binding multiple times on the same (address, port) combination.")
            } else {
                None
            };
            if let Some(m) = message {
                drop(reg);
                return OsError { code: -1, message: m.to_string() }.to_dart();
            }
            existing.uses_list += 1;
            let fd = existing.descriptor;
            let p = NativeSocket::new(retain_descriptor(fd));
            reg.per_socket.insert(p, fd);
            drop(reg);
            reuse_socket_on_object(this, p, SocketFinalizer::Listen);
            return dart_bool(true);
        }
    }
    let fd = match create_listener(&e, queue, v6_only) {
        Ok(fd) => fd,
        Err(error) => {
            drop(reg);
            return error.to_dart();
        }
    };
    let allocated_port = local_port(fd);
    let p = NativeSocket::new(fd);
    reg.sockets.push(ListeningSocket {
        address: e.ip(),
        port: allocated_port,
        v6_only,
        shared,
        uses_list: 1,
        descriptor: fd,
        unix_path: None,
    });
    reg.per_socket.insert(p, fd);
    drop(reg);
    reuse_socket_on_object(this, p, SocketFinalizer::Listen);
    dart_bool(true)
}

/// `ServerSocket_CreateUnixDomainBindListen`: como o de TCP, com o
/// compartilhamento pelo caminho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ServerSocket_CreateUnixDomainBindListen(this: i64, file_path: i64, queue: i64, shared: u8, _ns: i64) -> i64 {
    let file_path = utf8_of_string(file_path);
    let shared = shared != 0;
    let mut reg = listen_record();
    if let Some(existing) = reg.sockets.iter_mut().find(|s| s.unix_path.as_deref() == Some(&file_path[..])) {
        if !existing.shared || !shared {
            drop(reg);
            return OsError {
                code: -1,
                message: "The shared flag to bind() needs to be `true` if binding multiple times on the same path.".to_string(),
            }
            .to_dart();
        }
        existing.uses_list += 1;
        let fd = existing.descriptor;
        let p = NativeSocket::new(retain_descriptor(fd));
        reg.per_socket.insert(p, fd);
        drop(reg);
        reuse_socket_on_object(this, p, SocketFinalizer::Listen);
        return dart_bool(true);
    }
    let fd = match OsAddress::from_unix_path(&file_path).and_then(|e| create_unix_listener(&e, queue)) {
        Ok(fd) => fd,
        Err(e) => {
            drop(reg);
            return e.to_dart();
        }
    };
    let p = NativeSocket::new(fd);
    reg.sockets.push(ListeningSocket {
        address: Vec::new(),
        port: 0,
        v6_only: false,
        shared,
        uses_list: 1,
        descriptor: fd,
        unix_path: Some(file_path),
    });
    reg.per_socket.insert(p, fd);
    drop(reg);
    reuse_socket_on_object(this, p, SocketFinalizer::Listen);
    dart_bool(true)
}

/// `ServerSocket_Accept(socket)`: `true` com a conexão nova no objeto.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ServerSocket_Accept(this: i64, new: i64) -> u8 {
    let Some(s) = object_socket(this) else { return 0 };
    let fd = accept_conn(s.descriptor());
    if fd >= 0 {
        set_socket_on_object(new, fd, SocketFinalizer::Normal);
        return 1;
    }
    0
}

/// `Socket_Available`: os bytes prontos (1 se o sistema não sabe dizer).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_Available(this: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    let n = available(s);
    if n >= 0 { n } else { 1 }
}

/// `Socket_Read(len)`: os bytes lidos, `null` se nada veio; lança o
/// `OSError`. Lidos todos os `len` pedidos a lista volta, mesmo vazia com
/// `len` 0 (`socket.cc`, `bytes_read == length`): o `read()` sem contagem
/// do `_NativeSocket` de um soquete sem eventos (sem `listen`, `available`
/// em 0) faz `nativeRead(0)`, recebe a lista vazia, relê o `available` e
/// segue lendo. Isso sem asserts; com eles (o oráculo, `--enable-asserts`),
/// o `assert(available > 0)` lança antes e o soquete fecha, na VM também.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_Read(this: i64, n: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    if n < 0 {
        throw_os_error(&OsError::invalid_argument());
        return 0;
    }
    let mut buf = vec![0u8; n as usize];
    match read_from(s, &mut buf) {
        Ok(read_list) if read_list as i64 == n => dart_bytes(buf),
        Ok(0) => 0,
        Ok(read_list) => {
            buf.truncate(read_list);
            dart_bytes(buf)
        }
        Err(e) => {
            throw_os_error(&e);
            0
        }
    }
}

/// `Socket_WriteList(buffer, offset, bytes)`: os bytes escritos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_WriteList(this: i64, buffer: i64, start: i64, n: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    let bytes = typed_list_bytes(buffer).unwrap_or_default();
    let start = (start.max(0) as usize).min(bytes.len());
    let end = (start + n.max(0) as usize).min(bytes.len());
    match write_at(s, &bytes[start..end]) {
        Ok(written) => written as i64,
        Err(e) => {
            throw_os_error(&e);
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_HasPendingWrite(this: i64) -> u8 {
    object_socket(this).map_or(0, |s| u8::from(pending_write(s)))
}

/// `Socket_GetPort`: a porta local, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetPort(this: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    let p = local_port(s.descriptor());
    if p > 0 {
        return dart_int(p);
    }
    last_error().to_dart()
}

/// `Socket_GetRemotePeer`: `[[tipo, text, bytes], port]` (`[tipo,
/// texto]` no domínio Unix).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetRemotePeer(this: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    match remote_peer(s.descriptor()) {
        Ok((e, port)) => {
            let tipo = e.tipo();
            let text = allocate_str(&e.text());
            let input = with_roots(&[text], || {
                if tipo == TYPE_UNIX {
                    dart_fixed_list(&[dart_int(tipo), text])
                } else {
                    let ip = dart_bytes(e.ip());
                    with_roots(&[ip], || dart_fixed_list(&[dart_int(tipo), text, ip]))
                }
            });
            with_roots(&[input], || dart_fixed_list(&[input, dart_int(port)]))
        }
        Err(e) => {
            throw_os_error(&e);
            0
        }
    }
}

/// `Socket_GetError`: o erro pendente como `OSError`, ou `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetError(this: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    let c = pending_error(s.descriptor());
    if c != 0 { OsError::from_code(c).to_dart() } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetFD(this: i64) -> i64 {
    object_socket(this).map_or(0, NativeSocket::descriptor)
}

/// `_getSocketType(nativeSocket)`: o tipo do descritor (terminal, pipe,
/// arquivo, outro) ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetType(sock: i64) -> i64 {
    let Some(s) = object_socket(sock) else { return 0 };
    dart_or_error(descriptor_type(s.descriptor()), dart_int)
}

/// `Socket_GetStdioHandle(socket, num)`: a entrada/saída padrão `num` como
/// soquete (a entrada assíncrona de `stdin`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetStdioHandle(sock: i64, num: i64) -> u8 {
    let fd = default_handle(num.clamp(0, 2));
    if fd >= 0 {
        set_socket_on_object(sock, fd, SocketFinalizer::Stdio);
    }
    u8::from(fd >= 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetSocketId(this: i64) -> i64 {
    let p = native_field(this);
    if p == 0 {
        throw_internal_error("No native peer");
    }
    p
}

/// `Socket_SetSocketId(id, typeFlags)`: o objeto passa a usar o descritor
/// `id` (o `_NativeSocket` criado para um descritor já aberto: os pipes dos
/// sinais).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SetSocketId(this: i64, id: i64, flag_bits: i64) {
    let f = if flag_bits & (1 << 21) != 0 { SocketFinalizer::Signal } else { SocketFinalizer::Normal };
    set_socket_on_object(this, id, f);
}

/// `SocketBase_IsBindError(errno)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SocketBase_IsBindError(_this: i64, error: i64) -> u8 {
    u8::from(is_bind_error(error))
}

/// `OSError_inProgressErrorCode`: o "operação em andamento" do sistema.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_OSError_inProgressErrorCode() -> i64 {
    in_progress_code()
}

/// `Socket_GetOption(option, protocol)`: `TCP_NODELAY`, laço e TTL de
/// multicast, `SO_BROADCAST`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetOption(this: i64, option: i64, protocol: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    match get_option(s.descriptor(), option, protocol == TYPE_IPV4) {
        Some(v) if option == 2 => dart_int(v),
        Some(v) => dart_bool(v != 0),
        None => {
            throw_os_error(&last_error());
            0
        }
    }
}

/// `Socket_SetOption(option, protocol, value)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SetOption(this: i64, option: i64, protocol: i64, value: i64) {
    let Some(s) = object_socket(this) else { return };
    if !(0..=4).contains(&option) || option == 3 {
        throw_argument_error("option to setOption() is outside expected range");
        return;
    }
    let v = HEAP.with(|h| {
        let h = h.borrow();
        match h.bool_of(value) {
            Some(b) => i64::from(b),
            None => h.int_of(value).unwrap_or(0),
        }
    });
    if !set_option(s.descriptor(), option, protocol == TYPE_IPV4, v) {
        throw_os_error(&last_error());
    }
}

/// `Socket_SetRawOption(level, option, data)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SetRawOption(this: i64, level: i64, option: i64, data: i64) {
    let Some(s) = object_socket(this) else { return };
    let b = typed_list_bytes(data).unwrap_or_default();
    if !set_raw_option(s.descriptor(), level, option, &b) {
        throw_os_error(&last_error());
    }
}

/// `Socket_JoinMulticast(addr, interfaceAddr, interfaceIndex)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_JoinMulticast(this: i64, addr: i64, iface: i64, ix: i64) {
    multicast(this, addr, iface, ix, true);
}

/// `Socket_LeaveMulticast(addr, interfaceAddr, interfaceIndex)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_LeaveMulticast(this: i64, addr: i64, iface: i64, ix: i64) {
    multicast(this, addr, iface, ix, false);
}

/// Entra (`join`) ou sai de um grupo multicast, como a VM em cada sistema
/// (`socket_base_{linux,macos,win}.cc`): no Linux, `MCAST_JOIN_GROUP` com o
/// `group_req` (o índice da interface e o endereço do grupo); no macOS e no
/// Windows, `ip_mreq` (IPv4, pelo endereço da interface) ou `ipv6_mreq`
/// (IPv6, pelo índice). O endereço do grupo são os 4 ou 16 bytes do
/// `_InternetAddress._in_addr`.
fn multicast(this: i64, addr: i64, iface: i64, ix: i64, join: bool) {
    let Some(s) = object_socket(this) else { return };
    let Some(group) = typed_list_bytes(addr).filter(|b| b.len() == 4 || b.len() == 16) else {
        throw_argument_error("Invalid multicast address");
        return;
    };
    let iface = if iface == 0 { None } else { typed_list_bytes(iface) };
    let v4 = group.len() == 4;
    let (level, option, data) = multicast_option(&group, iface.as_deref(), ix, v4, join);
    if !set_raw_option(s.descriptor(), level, option, &data) {
        throw_os_error(&last_error());
    }
}

/// `IPPROTO_IP` e `IPPROTO_IPV6` (os mesmos nos três sistemas).
const LEVEL_IP: i64 = 0;
const LEVEL_IPV6: i64 = 41;

#[cfg(any(target_os = "linux", target_os = "android"))]
fn multicast_option(group: &[u8], _iface: Option<&[u8]>, ix: i64, v4: bool, join: bool) -> (i64, i64, Vec<u8>) {
    // `struct group_req { uint32_t gr_interface; struct sockaddr_storage
    // gr_group; }`: o `sockaddr_storage` alinhado em 8 (136 bytes).
    const MCAST_JOIN_GROUP: i64 = 42;
    const MCAST_LEAVE_GROUP: i64 = 45;
    let mut r = vec![0u8; 8 + 128];
    r[0..4].copy_from_slice(&(ix as u32).to_ne_bytes());
    if v4 {
        r[8..10].copy_from_slice(&2u16.to_ne_bytes()); // AF_INET
        r[12..16].copy_from_slice(group);
    } else {
        r[8..10].copy_from_slice(&10u16.to_ne_bytes()); // AF_INET6
        r[16..32].copy_from_slice(group);
    }
    let option = if join { MCAST_JOIN_GROUP } else { MCAST_LEAVE_GROUP };
    (if v4 { LEVEL_IP } else { LEVEL_IPV6 }, option, r)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn multicast_option(group: &[u8], iface: Option<&[u8]>, ix: i64, v4: bool, join: bool) -> (i64, i64, Vec<u8>) {
    // `IP_ADD_MEMBERSHIP`/`IP_DROP_MEMBERSHIP` e `IPV6_JOIN_GROUP`/
    // `IPV6_LEAVE_GROUP` valem 12 e 13 no macOS e no Windows.
    let option = if join { 12 } else { 13 };
    if v4 {
        // `struct ip_mreq { in_addr imr_multiaddr; in_addr imr_interface; }`
        let mut r = vec![0u8; 8];
        r[0..4].copy_from_slice(group);
        if let Some(i) = iface.filter(|i| i.len() == 4) {
            r[4..8].copy_from_slice(i);
        }
        (LEVEL_IP, option, r)
    } else {
        // `struct ipv6_mreq { in6_addr ipv6mr_multiaddr; unsigned int ipv6mr_interface; }`
        let mut r = vec![0u8; 20];
        r[0..16].copy_from_slice(group);
        r[16..20].copy_from_slice(&(ix as u32).to_ne_bytes());
        (LEVEL_IPV6, option, r)
    }
}

/// `Socket_GetRawOption(level, option, data)`: grava o valor em `data`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetRawOption(this: i64, level: i64, option: i64, data: i64) {
    let Some(s) = object_socket(this) else { return };
    let mut b = typed_list_bytes(data).unwrap_or_default();
    if !get_raw_option(s.descriptor(), level, option, &mut b) {
        throw_os_error(&last_error());
        return;
    }
    write_bytes_to_list(data, 0, &b);
}

/// `RawSocketOption_GetOptionValue(key)`: as constantes do sistema.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_RawSocketOption_GetOptionValue(key: i64) -> i64 {
    match raw_option_constant(key) {
        Some(v) => v,
        None => {
            throw_argument_error("option to getOptionValue() is outside expected range");
            0
        }
    }
}

/// `Socket_Fatal(msg)`: erro irrecuperável do `dart:io` (a VM aborta).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_Fatal(msg: i64) {
    let text = if msg == 0 { String::from("(null)") } else { HEAP.with(|h| h.borrow().text(msg).map(|t| t.para_string()).unwrap_or_default()) };
    eprintln!("Fatal error in dart:io (socket): {text}");
    std::process::abort();
}

// ---------------------------------------------------------------------------
// UDP.

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateBindDatagram(this: i64, addr: i64, port: i64, reuse_address: u8, reuse_port: u8, ttl: i64) -> i64 {
    let Some(e) = dest_address(addr, port, 0) else {
        throw_invalid_address();
        return 0;
    };
    created_socket(this, create_datagram(&e, reuse_address != 0, reuse_port != 0, ttl), SocketFinalizer::Normal)
}

/// `Socket_AvailableDatagram`: se há um datagrama.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_AvailableDatagram(this: i64) -> u8 {
    let Some(s) = object_socket(this) else { return 0 };
    u8::from(has_datagram(s.descriptor()))
}

/// O tamanho máximo de um datagrama UDP (o buffer de recepção da VM).
const MAX_DATAGRAM_SIZE: usize = 65536;

/// `Socket_RecvFrom`: o `Datagram` recebido (pelo `_makeDatagram` do
/// patch), ou `null` se não havia nenhum.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_RecvFrom(this: i64) -> i64 {
    socket_recvfrom(this, false)
}

/// Recebe um datagrama com retorno Owned na saída normal e null na pending.
///
/// O receiver fica emprestado durante a chamada. Quando há dados, o helper
/// Dart deve estar registrado como callback Owned de aridade cinco. A cópia
/// do resultado ocorre antes de fechar o escopo nativo; não consome o token
/// do receiver nem limpa exceção. Pode alocar, coletar e executar Dart.
///
/// # Panics
/// Helper ausente ou sem registro Owned, retorno/escopo incompatível;
/// falhas internas abortam no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // Receiver without peer: a saída pending não entrega one owner.
/// let socket = dartforge_arc_objeto_owned_v1(123, 1);
/// assert_eq!(dartforge_arc_socket_recvfrom_owned_v1(socket), 0);
/// assert_ne!(dartforge_exception_pending(), 0);
/// dartforge_exception_clear();
/// dartforge_arc_release(socket);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_socket_recvfrom_owned_v1(this: i64) -> i64 {
    let scope = ArcUniformScope(dartforge_arc_escopo_nativo_abrir_v1());
    let result = socket_recvfrom(this, true);
    let result = if dartforge_exception_pending() != 0 {
        assert_eq!(result, 0, "recepção ARC pendente deve devolver null");
        0
    } else {
        dartforge_arc_retain(result);
        result
    };
    drop(scope);
    result
}

fn socket_recvfrom(this: i64, owned: bool) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    let (buf, e) = match receive_datagram(s.descriptor(), MAX_DATAGRAM_SIZE) {
        Ok(Some(x)) => x,
        Ok(None) => return 0,
        Err(error) => {
            throw_os_error(&error);
            return 0;
        }
    };
    let Some(f) = ajudante("_dartforgeDatagrama") else {
        panic!("bug do compilador: dart:io sem `_dartforgeDatagrama` registrado");
    };
    // SAFETY: registrada pela sobreposição de `dart:io` com a assinatura
    // (`Uint8List`, `String`, `Uint8List`, `int`, `int`).
    let g = { let dart_target: usize = f; move |a0: i64, a1: i64, a2: i64, a3: i64, a4: i64| -> i64 {
        if owned {
            let guard = arc_prepare_port(dart_target, 5)
                .expect("callback do datagrama ARC incompatível")
                .expect("callback do datagrama ARC sem registro Owned");
            arc_finish_port(Some(guard), dart_r5_invoke(dart_target, a0, a1, a2, a3, a4))
        } else {
            dart_r5(dart_target, a0, a1, a2, a3, a4)
        }
    } };
    let data = dart_bytes(buf);
    let text = with_roots(&[data], || allocate_str(&e.text()));
    let ip = with_roots(&[data, text], || dart_bytes(e.ip()));
    with_roots(&[data, text, ip], || g(data, text, ip, e.port(), e.tipo()))
}

/// `Socket_SendTo(buffer, offset, bytes, address, port)`: os bytes
/// enviados (0 se o soquete está cheio).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SendTo(this: i64, buffer: i64, start: i64, n: i64, addr: i64, port: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    let Some(e) = dest_address(addr, port, 0) else {
        throw_invalid_address();
        return 0;
    };
    let bytes = typed_list_bytes(buffer).unwrap_or_default();
    let start = (start.max(0) as usize).min(bytes.len());
    let end = (start + n.max(0) as usize).min(bytes.len());
    match send_datagram(s.descriptor(), &bytes[start..end], &e) {
        Ok(n) => n as i64,
        Err(error) => {
            throw_os_error(&error);
            0
        }
    }
}

// ---------------------------------------------------------------------------
// `InternetAddress`.

/// `InternetAddress_Parse(address)`: os bytes do endereço numérico, ou
/// `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_InternetAddress_Parse(text: i64) -> i64 {
    let t = HEAP.with(|h| h.borrow().text(text).map(|t| t.para_string()).unwrap_or_default());
    parse_address(&t).map_or(0, dart_bytes)
}

/// `InternetAddress_RawAddrToString(address)`: o texto numérico.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_InternetAddress_RawAddrToString(addr: i64) -> i64 {
    let Some(e) = address_from_list(addr) else {
        throw_invalid_address();
        return 0;
    };
    allocate_str(&e.text())
}

/// `InternetAddress_ParseScopedLinkLocalAddress(address)`: o escopo de um
/// IPv6 de enlace local com `%interface`, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_InternetAddress_ParseScopedLinkLocalAddress(text: i64) -> i64 {
    let t = HEAP.with(|h| h.borrow().text(text).map(|t| t.para_string()).unwrap_or_default());
    match resolve_name(&t, TYPE_IPV6) {
        Ok(v) if !v.is_empty() => dart_int(v[0].scope()),
        Ok(_) => OsError::from_code(os_code::INVALID).to_dart(),
        Err(e) => e.to_dart(),
    }
}

// ---------------------------------------------------------------------------
// Os pedidos de rede do IOService (32 lookup, 33 interfaces, 34 reverso).

/// `Socket::LookupRequest(host, type)`: `[0, [tipo, text, bytes, scope]…]`.
fn resolve_request(d: &[Portable]) -> Portable {
    let (Some(host), Some(tipo)) = (d.first().and_then(Portable::str), d.get(1).and_then(Portable::int)) else {
        return invalid_argument();
    };
    response(resolve_name(host, tipo), |v| {
        let mut l = vec![Portable::Int(0)];
        for e in v {
            l.push(Portable::List(vec![Portable::Int(e.tipo()), Portable::Str(e.text()), Portable::Bytes(e.ip()), Portable::Int(e.scope())]));
        }
        Portable::List(l)
    })
}

/// `Socket::ListInterfacesRequest(type)`: `[0, [tipo, texto, bytes, nome,
/// índice]…]`.
fn interfaces_request(d: &[Portable]) -> Portable {
    let Some(tipo) = d.first().and_then(Portable::int) else {
        return invalid_argument();
    };
    response(list_interfaces(tipo), |v| {
        let mut l = vec![Portable::Int(0)];
        for (e, name, ix) in v {
            l.push(Portable::List(vec![Portable::Int(e.tipo()), Portable::Str(e.text()), Portable::Bytes(e.ip()), Portable::Str(name), Portable::Int(ix)]));
        }
        Portable::List(l)
    })
}

/// `Socket::ReverseLookupRequest(address)`: o nome do endereço.
fn reverse_resolve_request(d: &[Portable]) -> Portable {
    let Some(e) = d.first().and_then(Portable::bytes).and_then(OsAddress::from_ip) else {
        return invalid_argument();
    };
    response(address_name(&e), Portable::Str)
}

// ---------------------------------------------------------------------------
// `RawSynchronousSocket` (`sync_socket.cc` da VM): um TCP bloqueante, sem o
// manipulador de eventos. O `std::net::TcpStream` do Rust é o mesmo soquete
// bloqueante nos três sistemas; ele mora numa caixa no campo nativo do
// `_NativeSynchronousSocket` e é fechado no `closeSync` ou na coleta.

/// O soquete síncrono do objeto (`None` depois do `closeSync`).
type SyncSocket = Option<std::net::TcpStream>;

fn sync_socket<'a>(object: i64) -> Option<&'a mut SyncSocket> {
    let p = native_field(object);
    if p == 0 {
        throw_internal_error("No native peer");
        return None;
    }
    // SAFETY: a caixa criada em `CreateConnectSync`, viva até a coleta do
    // objeto (o finalizador a solta); o isolado é uma thread só.
    Some(unsafe { &mut *(p as *mut SyncSocket) })
}

fn finalize_sync_socket(p: usize) {
    // SAFETY: a caixa de `CreateConnectSync`, solta só aqui.
    drop(unsafe { Box::from_raw(p as *mut SyncSocket) });
}

/// O `OSError` de um erro do sistema do `std`.
fn io_error(e: &std::io::Error) -> i64 {
    OsError::from_code(e.raw_os_error().unwrap_or(0)).to_dart()
}

/// `[tipo, text, bytes]` de um IP.
fn dart_ip_address(ip: std::net::IpAddr) -> i64 {
    let (tipo, bytes) = match ip {
        std::net::IpAddr::V4(a) => (TYPE_IPV4, a.octets().to_vec()),
        std::net::IpAddr::V6(a) => (TYPE_IPV6, a.octets().to_vec()),
    };
    let text = allocate_str(&ip.to_string());
    with_roots(&[text], || {
        let b = dart_bytes(bytes);
        with_roots(&[b], || dart_fixed_list(&[dart_int(tipo), text, b]))
    })
}

/// `SynchronousSocket_LookupRequest(host, type)`: `[[tipo, text, bytes]…]`
/// ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_LookupRequest(host: i64, tipo: i64) -> i64 {
    let host = String::from_utf8_lossy(&utf8_of_string(host)).into_owned();
    match resolve_name(&host, tipo) {
        Ok(v) => {
            let mut items = Vec::with_capacity(v.len());
            let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(v.len().max(1)));
            for (i, e) in v.iter().enumerate() {
                let text = allocate_str(&e.text());
                let item = with_roots(&[text], || {
                    let b = dart_bytes(e.ip());
                    with_roots(&[b], || dart_fixed_list(&[dart_int(e.tipo()), text, b]))
                });
                HEAP.with(|h| h.borrow_mut().set_root(frame, i, item));
                items.push(item);
            }
            let r = dart_fixed_list(&items);
            HEAP.with(|h| h.borrow_mut().pop_frame(frame));
            r
        }
        Err(e) => e.to_dart(),
    }
}

/// `SynchronousSocket_CreateConnectSync(addr, port)`: `null` conectado, ou
/// o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_CreateConnectSync(this: i64, addr: i64, port: i64) -> i64 {
    let ip = match typed_list_bytes(addr).as_deref() {
        Some(&[a, b, c, d]) => std::net::IpAddr::from([a, b, c, d]),
        Some(b) if b.len() == 16 => {
            let mut o = [0u8; 16];
            o.copy_from_slice(b);
            std::net::IpAddr::from(o)
        }
        _ => {
            throw_invalid_address();
            return 0;
        }
    };
    match std::net::TcpStream::connect((ip, port as u16)) {
        Ok(s) => {
            let p = Box::into_raw(Box::new(Some(s))) as i64;
            write_native_field(this, p);
            attach_finalizer(this, finalize_sync_socket, p as usize);
            0
        }
        Err(e) => io_error(&e),
    }
}

/// `SynchronousSocket_Available`: os bytes já recebidos e não lidos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_Available(this: i64) -> i64 {
    let Some(Some(s)) = sync_socket(this) else { return dart_int(0) };
    dart_int(ready_bytes(s).max(0))
}

#[cfg(unix)]
fn ready_bytes(s: &std::net::TcpStream) -> i64 {
    use std::os::fd::AsRawFd;
    available_bytes(i64::from(s.as_raw_fd()))
}

#[cfg(windows)]
fn ready_bytes(s: &std::net::TcpStream) -> i64 {
    use std::os::windows::io::AsRawSocket;
    #[link(name = "ws2_32")]
    unsafe extern "system" {
        fn ioctlsocket(s: usize, cmd: i32, argp: *mut u32) -> i32;
    }
    const FIONREAD: i32 = 0x4004_667F;
    let mut n: u32 = 0;
    // SAFETY: o soquete do `TcpStream`, vivo durante a chamada.
    if unsafe { ioctlsocket(s.as_raw_socket() as usize, FIONREAD, &mut n) } != 0 {
        return -1;
    }
    i64::from(n)
}

/// `SynchronousSocket_CloseSync`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_CloseSync(this: i64) {
    if let Some(s) = sync_socket(this) {
        s.take();
    }
}

/// `SynchronousSocket_GetPort`: a porta local ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_GetPort(this: i64) -> i64 {
    let Some(Some(s)) = sync_socket(this) else { return 0 };
    match s.local_addr() {
        Ok(a) => dart_int(i64::from(a.port())),
        Err(e) => io_error(&e),
    }
}

/// `SynchronousSocket_GetRemotePeer`: `[[tipo, text, bytes], port]` ou o
/// `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_GetRemotePeer(this: i64) -> i64 {
    let Some(Some(s)) = sync_socket(this) else { return 0 };
    match s.peer_addr() {
        Ok(a) => {
            let e = dart_ip_address(a.ip());
            with_roots(&[e], || dart_fixed_list(&[e, dart_int(i64::from(a.port()))]))
        }
        Err(e) => io_error(&e),
    }
}

/// `SynchronousSocket_Read(len)`: até `len` bytes (bloqueia até chegar
/// algum); `null` no fim do fluxo; ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_Read(this: i64, len: i64) -> i64 {
    use std::io::Read;
    let Some(Some(s)) = sync_socket(this) else { return 0 };
    let len = len.max(0) as usize;
    let mut buf = vec![0u8; len];
    match s.read(&mut buf) {
        Ok(0) => 0,
        Ok(n) => {
            buf.truncate(n);
            dart_bytes(buf)
        }
        Err(e) => io_error(&e),
    }
}

/// `SynchronousSocket_ReadList(buffer, offset, bytes)`: grava em `buffer`
/// e devolve quantos leu (0 no fim do fluxo), ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_ReadList(this: i64, buffer: i64, start: i64, n: i64) -> i64 {
    use std::io::Read;
    let Some(Some(s)) = sync_socket(this) else { return 0 };
    let start = start.max(0) as usize;
    let n = n.max(0) as usize;
    let mut buf = vec![0u8; n];
    match s.read(&mut buf) {
        Ok(read_list) => {
            write_bytes_to_list(buffer, start, &buf[..read_list]);
            dart_int(read_list as i64)
        }
        Err(e) => io_error(&e),
    }
}

/// `SynchronousSocket_WriteList(buffer, offset, bytes)`: escreve tudo e
/// devolve quantos, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_WriteList(this: i64, buffer: i64, start: i64, n: i64) -> i64 {
    use std::io::Write;
    let Some(Some(s)) = sync_socket(this) else { return 0 };
    let start = start.max(0) as usize;
    let n = n.max(0) as usize;
    let bytes = typed_list_bytes(buffer).unwrap_or_default();
    let start = start.min(bytes.len());
    let end = (start + n).min(bytes.len());
    match s.write_all(&bytes[start..end]) {
        Ok(()) => dart_int((end - start) as i64),
        Err(e) => io_error(&e),
    }
}

/// `SynchronousSocket_ShutdownRead`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_ShutdownRead(this: i64) {
    if let Some(Some(s)) = sync_socket(this) {
        let _ = s.shutdown(std::net::Shutdown::Read);
    }
}

/// `SynchronousSocket_ShutdownWrite`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_ShutdownWrite(this: i64) {
    if let Some(Some(s)) = sync_socket(this) {
        let _ = s.shutdown(std::net::Shutdown::Write);
    }
}

// ---------------------------------------------------------------------------
// Mensagens de controle e recursos passados por soquete de domínio Unix
// (`SocketControlMessage`, `ResourceHandle`; N04). Os objetos Dart saem dos
// ajudantes `_dartforge*` de `native_sdk/io/common_patch.dart`; no Windows,
// como na VM, não há passagem de descritores.

/// A função Dart registrada `name` (da sobreposição de `dart:io`).
#[cfg(unix)]
fn ajudante_de_io(name: &str) -> usize {
    ajudante(name).unwrap_or_else(|| panic!("bug do compilador: dart:io sem `{name}` registrado"))
}

/// O `int` de um elemento de `List<dynamic>` (escalar ou caixa).
#[cfg(unix)]
fn int_from_list(list: i64, i: usize) -> Option<i64> {
    HEAP.with(|h| {
        let h = h.borrow();
        match h.list_get(list, i) {
            crate::heap::Value::Int(v) => Some(v),
            crate::heap::Value::Ref(r) => h.int_of(r),
            _ => None,
        }
    })
}

/// A `UnsupportedError` dos recursos por soquete fora do Unix.
#[cfg(windows)]
fn throw_out_of_resources() {
    let msg = allocate_str("This is not supported on this operating system");
    let e = with_roots(&[msg], || dartforge_unsupported_error_new(msg));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

/// `SocketControlMessage.fromHandles(handles)`: a mensagem `SCM_RIGHTS`
/// com os descritores.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SocketControlMessage_fromHandles(resources: i64) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `(List<ResourceHandle>, int, int) → Object`.
        let f = { let dart_target: usize = ajudante_de_io("_dartforgeMensagemDeRecursos"); move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(dart_target, a0, a1, a2) } };
        let (level, tipo) = resource_level_and_type();
        f(resources, level, tipo)
    }
    #[cfg(windows)]
    {
        let _ = resources;
        throw_out_of_resources();
        0
    }
}

/// `_SocketControlMessageImpl.extractHandles()`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SocketControlMessageImpl_extractHandles(this: i64) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `(Object, int, int) → List<ResourceHandle>`.
        let f = { let dart_target: usize = ajudante_de_io("_dartforgeRecursosDaMensagem"); move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(dart_target, a0, a1, a2) } };
        let (level, tipo) = resource_level_and_type();
        f(this, level, tipo)
    }
    #[cfg(windows)]
    {
        let _ = this;
        throw_out_of_resources();
        0
    }
}

/// `Socket_SendMessage(buffer, offset, bytes, controlMessages)`: os bytes
/// enviados; `controlMessages` é `[nível, tipo, Uint8List]*`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SendMessage(this: i64, buffer: i64, start: i64, n: i64, controls: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    #[cfg(unix)]
    {
        let bytes = typed_list_bytes(buffer).unwrap_or_default();
        let start = (start.max(0) as usize).min(bytes.len());
        let end = (start + n.max(0) as usize).min(bytes.len());
        let total = HEAP.with(|h| h.borrow().list_len(controls));
        let mut messages = Vec::with_capacity(total / 3);
        for i in (0..total - total % 3).step_by(3) {
            let data = HEAP.with(|h| h.borrow().list_get(controls, i + 2));
            let (data_ref, data_and_ref) = match data {
                crate::heap::Value::Ref(r) => (r, true),
                _ => (0, false),
            };
            match (int_from_list(controls, i), int_from_list(controls, i + 1), typed_list_bytes(data_ref)) {
                (Some(level), Some(tipo), Some(d)) if data_and_ref => messages.push((level, tipo, d)),
                _ => {
                    throw_argument_error("Invalid control message");
                    return 0;
                }
            }
        }
        match send_message(s.descriptor(), &bytes[start..end], &messages) {
            Ok(n) => dart_int(n as i64),
            Err(e) => {
                throw_os_error(&e);
                0
            }
        }
    }
    #[cfg(windows)]
    {
        let _ = (s, buffer, start, n, controls);
        throw_out_of_resources();
        0
    }
}

/// `Socket_ReceiveMessage(len)`: `[nível, tipo, Uint8List]*` e, por último,
/// a `Uint8List` dos dados.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_ReceiveMessage(this: i64, n: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    #[cfg(unix)]
    {
        let (data, messages) = match receive_message(s.descriptor(), n.max(0) as usize) {
            Ok(x) => x,
            Err(e) => {
                throw_os_error(&e);
                return 0;
            }
        };
        let mut items: Vec<i64> = Vec::with_capacity(messages.len() * 3 + 1);
        for (level, tipo, d) in messages {
            let b = with_roots(&items, || dart_bytes(d));
            items.extend([dart_int(level), dart_int(tipo), b]);
        }
        let b = with_roots(&items, || dart_bytes(data));
        items.push(b);
        with_roots(&items, || dart_fixed_list(&items))
    }
    #[cfg(windows)]
    {
        let _ = (s, n);
        throw_out_of_resources();
        0
    }
}

/// `ResourceHandleImpl_toFile`: o `RandomAccessFile` sobre o descritor (o
/// arquivo passa a ser dono dele).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ResourceHandleImpl_toFile(this: i64) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `(Object) → int` e `(int) → RandomAccessFile`.
        let descriptor = { let dart_target: usize = ajudante_de_io("_dartforgeDescritorDoRecurso"); move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        let file = { let dart_target: usize = ajudante_de_io("_dartforgeArquivoDoRecurso"); move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        file(NativeFile::new(descriptor(this)))
    }
    #[cfg(windows)]
    {
        let _ = this;
        throw_out_of_resources();
        0
    }
}

/// `ResourceHandleImpl_toRawSocket`: `[tipo, text, bytes, fd]` do endereço
/// local do descritor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ResourceHandleImpl_toRawSocket(this: i64) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `(Object) → int`.
        let descriptor = { let dart_target: usize = ajudante_de_io("_dartforgeDescritorDoRecurso"); move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        let fd = descriptor(this);
        match local_address(fd) {
            Ok(e) => {
                let tipo = e.tipo();
                let text = e.text();
                let raw = if tipo == TYPE_UNIX { text.clone().into_bytes() } else { e.ip() };
                let t = allocate_str(&text);
                let b = with_roots(&[t], || dart_bytes(raw));
                with_roots(&[t, b], || dart_fixed_list(&[dart_int(tipo), t, b, dart_int(fd)]))
            }
            Err(e) => {
                throw_os_error(&e);
                0
            }
        }
    }
    #[cfg(windows)]
    {
        let _ = this;
        throw_out_of_resources();
        0
    }
}

/// `ResourceHandleImpl_toSocket` e `_toRawDatagramSocket`: a VM não os
/// suporta (o native dela devolve um `UnsupportedError`, que o retorno
/// tipado transforma em `TypeError`); aqui a `UnsupportedError` é lançada.
fn throw_unsupported_resource(api: &str) {
    let msg = allocate_str(&format!("ResourceHandle.{api} is not supported"));
    let e = with_roots(&[msg], || dartforge_unsupported_error_new(msg));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ResourceHandleImpl_toSocket(_this: i64) -> i64 {
    throw_unsupported_resource("toSocket");
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ResourceHandleImpl_toRawDatagramSocket(_this: i64) -> i64 {
    throw_unsupported_resource("toRawDatagramSocket");
    0
}
