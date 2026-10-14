// Native runtime: the VM's `dart:io` sockets (`runtime/bin/socket.cc`): TCP
// (client and server, with the registry of shared listening sockets), UDP, Unix
// domain, addresses (`InternetAddress`) and the IOService's name resolution
// (requests 32–34).
//
// The natives return what the VM's return (`true`, an `OSError`, a `Uint8List`…)
// and throw what they throw; the descriptor lives in the [`NativeSocket`] of the
// native field (`io_events.rs`), non-blocking, and the event handler notifies when
// there is something to read or write.
//
// The natives do not depend on the system: the system layer (`OsAddress`,
// `create_connection`, `read_from`, `resolve_name`…) has the same names in
// `io_sockets_unix.rs` (Linux and macOS) and in `io_windows_sockets.rs` (Winsock,
// on top of the completion port of `io_windows_events.rs`).

/// `SocketAddress::TYPE_*`.
const TYPE_IPV4: i64 = 0;
const TYPE_IPV6: i64 = 1;
const TYPE_UNIX: i64 = 2;

/// A `sockaddr_storage` and the size used.
#[derive(Clone)]
struct OsAddress {
    bytes: [u8; 128],
    size: u32,
}

/// The address of a Dart `Uint8List` (4 or 16 bytes).
fn address_from_list(h: i64) -> Option<OsAddress> {
    OsAddress::from_ip(&typed_list_bytes(h)?)
}

/// The `ArgumentError` of a malformed address (the VM propagates an API error:
/// "Unexpected type for socket address").
fn throw_invalid_address() {
    let msg = allocate_str("Unexpected type for socket address");
    let e = with_roots(&[msg], || dartforge_argument_error_new(msg, 0));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

/// Throws the `OSError` (the socket natives throw instead of returning).
fn throw_os_error(e: &OsError) {
    let error = e.to_dart();
    if dartforge_exception_pending() == 0 {
        with_roots(&[error], || dartforge_exception_throw(error, 3));
    }
}

/// Throws `ArgumentError(message)` (the VM's API errors).
fn throw_argument_error(message: &str) {
    let msg = allocate_str(message);
    let e = with_roots(&[msg], || dartforge_argument_error_new(msg, 0));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

// ---------------------------------------------------------------------------
// The `_NativeSocket` natives.

/// The answer of a creation native: `true` with the socket in the object, or the
/// `OSError`.
fn created_socket(object: i64, r: IoResult<i64>, finalizer: SocketFinalizer) -> i64 {
    match r {
        Ok(fd) => {
            set_socket_on_object(object, fd, finalizer);
            dart_bool(true)
        }
        Err(e) => e.to_dart(),
    }
}

/// The connection address: the IP of `addr`, `port` and the IPv6 scope.
fn dest_address(addr: i64, port: i64, scope: i64) -> Option<OsAddress> {
    let mut e = address_from_list(addr)?;
    e.write_port(port);
    e.write_scope(scope);
    Some(e)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_CreateConnect(this: i64, addr: i64, port: i64, scope: i64) -> i64 {
    let Some(e) = dest_address(addr, port, scope) else {
        throw_invalid_address();
        return 0;
    };
    created_socket(this, create_connection(&e), SocketFinalizer::Normal)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_CreateBindConnect(this: i64, addr: i64, port: i64, origin: i64, source_port: i64, scope: i64) -> i64 {
    let (Some(e), Some(o)) = (dest_address(addr, port, scope), dest_address(origin, source_port, 0)) else {
        throw_invalid_address();
        return 0;
    };
    created_socket(this, create_connection_with_source(&e, &o), SocketFinalizer::Normal)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_CreateUnixDomainConnect(this: i64, file_path: i64, _ns: i64) -> i64 {
    let r = OsAddress::from_unix_path(&utf8_of_string(file_path)).and_then(|e| create_unix_connection(&e, None));
    created_socket(this, r, SocketFinalizer::Normal)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_CreateUnixDomainBindConnect(this: i64, file_path: i64, origin: i64, _ns: i64) -> i64 {
    let r = OsAddress::from_unix_path(&utf8_of_string(file_path)).and_then(|e| {
        let o = OsAddress::from_unix_path(&utf8_of_string(origin))?;
        create_unix_connection(&e, Some(&o))
    });
    created_socket(this, r, SocketFinalizer::Normal)
}

/// `ServerSocket_CreateBindListen`: the listening socket, shared with another one of
/// the same (address, port) when both ask for `shared`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_ServerSocket_CreateBindListen(
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

/// `ServerSocket_CreateUnixDomainBindListen`: like the TCP one, sharing by path.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_ServerSocket_CreateUnixDomainBindListen(this: i64, file_path: i64, queue: i64, shared: u8, _ns: i64) -> i64 {
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

/// `ServerSocket_Accept(socket)`: `true` with the new connection in the object.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_ServerSocket_Accept(this: i64, new: i64) -> u8 {
    let Some(s) = object_socket(this) else { return 0 };
    let fd = accept_conn(s.descriptor());
    if fd >= 0 {
        set_socket_on_object(new, fd, SocketFinalizer::Normal);
        return 1;
    }
    0
}

/// `Socket_Available`: the ready bytes (1 if the system cannot tell).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_Available(this: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    let n = available(s);
    if n >= 0 { n } else { 1 }
}

/// `Socket_Read(len)`: the bytes read, `null` if nothing came; throws the `OSError`.
/// When all the `len` requested bytes were read the list comes back, even empty
/// with `len` 0 (`socket.cc`, `bytes_read == length`): the count-less `read()` of
/// the `_NativeSocket` of a socket without events (no `listen`, `available` at 0)
/// does `nativeRead(0)`, receives the empty list, reads `available` again and goes
/// on reading. That is without asserts; with them (the oracle,
/// `--enable-asserts`), `assert(available > 0)` throws first and the socket
/// closes, in the VM too.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_Read(this: i64, n: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    if n < 0 {
        throw_os_error(&OsError::invalid_argument());
        return 0;
    }
    // As in the SDK's Socket_Read: read directly into the Dart buffer, then
    // copy only a short read to an exactly sized result. Root the receiver
    // before allocation so collection cannot finalize the native socket.
    let (buffer, result) = with_roots(&[this], || HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let buffer = heap.new_typed(TYPE_UINT8, n as usize);
        let result = read_from(s, heap.typed_bytes_mut(buffer).unwrap());
        (buffer, result)
    }));
    match result {
        Ok(read_list) if read_list as i64 == n => buffer,
        Ok(0) => 0,
        Ok(read_list) => with_roots(&[buffer], || HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            let short = heap.new_typed(TYPE_UINT8, read_list);
            let source = heap.typed(buffer).unwrap();
            let dest = heap.typed(short).unwrap();
            // SAFETY: distinct, internal buffers rooted across allocation;
            // read_from returned at most the destination's requested length.
            // The exclusive heap borrow prevents collection and other access.
            unsafe { std::ptr::copy_nonoverlapping(source.data, dest.data, read_list); }
            short
        })),
        Err(e) => {
            throw_os_error(&e);
            0
        }
    }
}

/// `Socket_WriteList(buffer, offset, bytes)`: the bytes written.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_WriteList(this: i64, buffer: i64, start: i64, n: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    // Keep the Dart storage alive until the OS has consumed the slice. Unix
    // writes synchronously; Windows copies the submitted slice into its owned
    // overlapped operation before returning. Neither path calls Dart or collects.
    // Release the heap borrow before constructing an error (which can allocate).
    let result = HEAP.with(|heap| {
        let heap = heap.borrow();
        let bytes = heap.typed_bytes(buffer).unwrap_or_default();
        let start = (start.max(0) as usize).min(bytes.len());
        let end = start.saturating_add(n.max(0) as usize).min(bytes.len());
        write_at(s, &bytes[start..end])
    });
    match result {
        Ok(written) => written as i64,
        Err(e) => {
            throw_os_error(&e);
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_HasPendingWrite(this: i64) -> u8 {
    object_socket(this).map_or(0, |s| u8::from(pending_write(s)))
}

/// `Socket_GetPort`: the local port, or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_GetPort(this: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    let p = local_port(s.descriptor());
    if p > 0 {
        return dart_int(p);
    }
    last_error().to_dart()
}

/// `Socket_GetRemotePeer`: `[[type, text, bytes], port]` (`[type, text]` in the
/// Unix domain).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_GetRemotePeer(this: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    match remote_peer(s.descriptor()) {
        Ok((e, port)) => {
            let ty = e.ty();
            let text = allocate_str(&e.text());
            let input = with_roots(&[text], || {
                if ty == TYPE_UNIX {
                    dart_fixed_list(&[dart_int(ty), text])
                } else {
                    let ip = dart_bytes(e.ip());
                    with_roots(&[ip], || dart_fixed_list(&[dart_int(ty), text, ip]))
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

/// `Socket_GetError`: the pending error as an `OSError`, or `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_GetError(this: i64) -> i64 {
    let Some(s) = object_socket(this) else { return 0 };
    let c = pending_error(s.descriptor());
    if c != 0 { OsError::from_code(c).to_dart() } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_GetFD(this: i64) -> i64 {
    object_socket(this).map_or(0, NativeSocket::descriptor)
}

/// `_getSocketType(nativeSocket)`: the descriptor type (terminal, pipe, file,
/// other) or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_GetType(sock: i64) -> i64 {
    let Some(s) = object_socket(sock) else { return 0 };
    dart_or_error(descriptor_type(s.descriptor()), dart_int)
}

/// `Socket_GetStdioHandle(socket, num)`: the standard input/output `num` as a
/// socket (the asynchronous input of `stdin`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_GetStdioHandle(sock: i64, num: i64) -> u8 {
    let fd = default_handle(num.clamp(0, 2));
    if fd >= 0 {
        set_socket_on_object(sock, fd, SocketFinalizer::Stdio);
    }
    u8::from(fd >= 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_GetSocketId(this: i64) -> i64 {
    let p = native_field(this);
    if p == 0 {
        throw_internal_error("No native peer");
    }
    p
}

/// `Socket_SetSocketId(id, typeFlags)`: the object starts using descriptor `id`
/// (the `_NativeSocket` created for an already open descriptor: the signal pipes).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_SetSocketId(this: i64, id: i64, flag_bits: i64) {
    let f = if flag_bits & (1 << 21) != 0 { SocketFinalizer::Signal } else { SocketFinalizer::Normal };
    set_socket_on_object(this, id, f);
}

/// `SocketBase_IsBindError(errno)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SocketBase_IsBindError(_this: i64, error: i64) -> u8 {
    u8::from(is_bind_error(error))
}

/// `OSError_inProgressErrorCode`: the system's "operation in progress".
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_OSError_inProgressErrorCode() -> i64 {
    in_progress_code()
}

/// `Socket_GetOption(option, protocol)`: `TCP_NODELAY`, multicast loop and TTL,
/// `SO_BROADCAST`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_GetOption(this: i64, option: i64, protocol: i64) -> i64 {
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
pub extern "C" fn dartforge_native_Socket_SetOption(this: i64, option: i64, protocol: i64, value: i64) {
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
pub extern "C" fn dartforge_native_Socket_SetRawOption(this: i64, level: i64, option: i64, data: i64) {
    let Some(s) = object_socket(this) else { return };
    let b = typed_list_bytes(data).unwrap_or_default();
    if !set_raw_option(s.descriptor(), level, option, &b) {
        throw_os_error(&last_error());
    }
}

/// `Socket_JoinMulticast(addr, interfaceAddr, interfaceIndex)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_JoinMulticast(this: i64, addr: i64, iface: i64, ix: i64) {
    multicast(this, addr, iface, ix, true);
}

/// `Socket_LeaveMulticast(addr, interfaceAddr, interfaceIndex)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_LeaveMulticast(this: i64, addr: i64, iface: i64, ix: i64) {
    multicast(this, addr, iface, ix, false);
}

/// Joins (`join`) or leaves a multicast group, like the VM on each system
/// (`socket_base_{linux,macos,win}.cc`): on Linux, `MCAST_JOIN_GROUP` with the
/// `group_req` (the interface index and the group address); on macOS and Windows,
/// `ip_mreq` (IPv4, by interface address) or `ipv6_mreq` (IPv6, by index). The
/// group address is the 4 or 16 bytes of `_InternetAddress._in_addr`.
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

/// `IPPROTO_IP` and `IPPROTO_IPV6` (the same on the three systems).
const LEVEL_IP: i64 = 0;
const LEVEL_IPV6: i64 = 41;

#[cfg(any(target_os = "linux", target_os = "android"))]
fn multicast_option(group: &[u8], _iface: Option<&[u8]>, ix: i64, v4: bool, join: bool) -> (i64, i64, Vec<u8>) {
    // `struct group_req { uint32_t gr_interface; struct sockaddr_storage
    // gr_group; }`: the `sockaddr_storage` aligned to 8 (136 bytes).
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
    // `IP_ADD_MEMBERSHIP`/`IP_DROP_MEMBERSHIP` and `IPV6_JOIN_GROUP`/
    // `IPV6_LEAVE_GROUP` are 12 and 13 on macOS and Windows.
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

/// `Socket_GetRawOption(level, option, data)`: writes the value into `data`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_GetRawOption(this: i64, level: i64, option: i64, data: i64) {
    let Some(s) = object_socket(this) else { return };
    let mut b = typed_list_bytes(data).unwrap_or_default();
    if !get_raw_option(s.descriptor(), level, option, &mut b) {
        throw_os_error(&last_error());
        return;
    }
    write_bytes_to_list(data, 0, &b);
}

/// `RawSocketOption_GetOptionValue(key)`: the system constants.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_RawSocketOption_GetOptionValue(key: i64) -> i64 {
    match raw_option_constant(key) {
        Some(v) => v,
        None => {
            throw_argument_error("option to getOptionValue() is outside expected range");
            0
        }
    }
}

/// `Socket_Fatal(msg)`: an unrecoverable `dart:io` error (the VM aborts).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_Fatal(msg: i64) {
    let text = if msg == 0 { String::from("(null)") } else { HEAP.with(|h| h.borrow().text(msg).map(|t| t.to_dart_string()).unwrap_or_default()) };
    eprintln!("Fatal error in dart:io (socket): {text}");
    std::process::abort();
}

// ---------------------------------------------------------------------------
// UDP.

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_CreateBindDatagram(this: i64, addr: i64, port: i64, reuse_address: u8, reuse_port: u8, ttl: i64) -> i64 {
    let Some(e) = dest_address(addr, port, 0) else {
        throw_invalid_address();
        return 0;
    };
    created_socket(this, create_datagram(&e, reuse_address != 0, reuse_port != 0, ttl), SocketFinalizer::Normal)
}

/// `Socket_AvailableDatagram`: whether there is a datagram.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_AvailableDatagram(this: i64) -> u8 {
    let Some(s) = object_socket(this) else { return 0 };
    u8::from(has_datagram(s.descriptor()))
}

/// The maximum size of a UDP datagram (the VM's receive buffer).
const MAX_DATAGRAM_SIZE: usize = 65536;

/// `Socket_RecvFrom`: the received `Datagram` (through the patch's
/// `_makeDatagram`), or `null` if there was none.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_RecvFrom(this: i64) -> i64 {
    socket_recvfrom(this, false)
}

/// Receives a datagram with an Owned return on the normal exit and null on the
/// pending one.
///
/// The receiver stays borrowed during the call. When there is data, the Dart
/// helper must be registered as an Owned callback of arity five. The result is
/// copied before the native scope closes; it does not consume the receiver's token
/// nor clear the exception. May allocate, collect and run Dart.
///
/// # Panics
/// Missing helper or without an Owned registration, an incompatible return/scope;
/// internal failures abort at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// // Receiver without a peer: the pending exit hands over no owner.
/// let socket = dartforge_arc_object_owned_v1(123, 1);
/// assert_eq!(dartforge_arc_socket_recvfrom_owned_v1(socket), 0);
/// assert_ne!(dartforge_exception_pending(), 0);
/// dartforge_exception_clear();
/// dartforge_arc_release(socket);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_socket_recvfrom_owned_v1(this: i64) -> i64 {
    let scope = ArcUniformScope(dartforge_arc_native_scope_open_v1());
    let result = socket_recvfrom(this, true);
    let result = if dartforge_exception_pending() != 0 {
        assert_eq!(result, 0, "pending ARC receive must return null");
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
    let Some(f) = helper("_dartforgeDatagram") else {
        panic!("compiler bug: dart:io without a registered `_dartforgeDatagram`");
    };
    // SAFETY: registered by the `dart:io` overlay with the signature (`Uint8List`,
    // `String`, `Uint8List`, `int`, `int`).
    let g = { let dart_target: usize = f; move |a0: i64, a1: i64, a2: i64, a3: i64, a4: i64| -> i64 {
        if owned {
            let guard = arc_prepare_port(dart_target, 5)
                .expect("incompatible ARC datagram callback")
                .expect("ARC datagram callback without an Owned registration");
            arc_finish_port(Some(guard), dart_r5_invoke(dart_target, a0, a1, a2, a3, a4))
        } else {
            dart_r5(dart_target, a0, a1, a2, a3, a4)
        }
    } };
    let data = dart_bytes(buf);
    let text = with_roots(&[data], || allocate_str(&e.text()));
    let ip = with_roots(&[data, text], || dart_bytes(e.ip()));
    with_roots(&[data, text, ip], || g(data, text, ip, e.port(), e.ty()))
}

/// `Socket_SendTo(buffer, offset, bytes, address, port)`: the bytes sent (0 if the
/// socket is full).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_SendTo(this: i64, buffer: i64, start: i64, n: i64, addr: i64, port: i64) -> i64 {
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

/// `InternetAddress_Parse(address)`: the bytes of the numeric address, or `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_InternetAddress_Parse(text: i64) -> i64 {
    let t = HEAP.with(|h| h.borrow().text(text).map(|t| t.to_dart_string()).unwrap_or_default());
    parse_address(&t).map_or(0, dart_bytes)
}

/// `InternetAddress_RawAddrToString(address)`: the numeric text.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_InternetAddress_RawAddrToString(addr: i64) -> i64 {
    let Some(e) = address_from_list(addr) else {
        throw_invalid_address();
        return 0;
    };
    allocate_str(&e.text())
}

/// `InternetAddress_ParseScopedLinkLocalAddress(address)`: the scope of a link-local
/// IPv6 with `%interface`, or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_InternetAddress_ParseScopedLinkLocalAddress(text: i64) -> i64 {
    let t = HEAP.with(|h| h.borrow().text(text).map(|t| t.to_dart_string()).unwrap_or_default());
    match resolve_name(&t, TYPE_IPV6) {
        Ok(v) if !v.is_empty() => dart_int(v[0].scope()),
        Ok(_) => OsError::from_code(os_code::INVALID).to_dart(),
        Err(e) => e.to_dart(),
    }
}

// ---------------------------------------------------------------------------
// The IOService's network requests (32 lookup, 33 interfaces, 34 reverse).

/// `Socket::LookupRequest(host, type)`: `[0, [type, text, bytes, scope]…]`.
fn resolve_request(d: &[Portable]) -> Portable {
    let (Some(host), Some(ty)) = (d.first().and_then(Portable::str), d.get(1).and_then(Portable::int)) else {
        return invalid_argument();
    };
    response(resolve_name(host, ty), |v| {
        let mut l = vec![Portable::Int(0)];
        for e in v {
            l.push(Portable::List(vec![Portable::Int(e.ty()), Portable::Str(e.text()), Portable::Bytes(e.ip()), Portable::Int(e.scope())]));
        }
        Portable::List(l)
    })
}

/// `Socket::ListInterfacesRequest(type)`: `[0, [type, text, bytes, name, index]…]`.
fn interfaces_request(d: &[Portable]) -> Portable {
    let Some(ty) = d.first().and_then(Portable::int) else {
        return invalid_argument();
    };
    response(list_interfaces(ty), |v| {
        let mut l = vec![Portable::Int(0)];
        for (e, name, ix) in v {
            l.push(Portable::List(vec![Portable::Int(e.ty()), Portable::Str(e.text()), Portable::Bytes(e.ip()), Portable::Str(name), Portable::Int(ix)]));
        }
        Portable::List(l)
    })
}

/// `Socket::ReverseLookupRequest(address)`: the address's name.
fn reverse_resolve_request(d: &[Portable]) -> Portable {
    let Some(e) = d.first().and_then(Portable::bytes).and_then(OsAddress::from_ip) else {
        return invalid_argument();
    };
    response(address_name(&e), Portable::Str)
}

// ---------------------------------------------------------------------------
// `RawSynchronousSocket` (the VM's `sync_socket.cc`): a blocking TCP socket,
// without the event handler. Rust's `std::net::TcpStream` is the same blocking
// socket on the three systems; it lives in a box in the native field of the
// `_NativeSynchronousSocket` and is closed on `closeSync` or on collection.

/// The object's synchronous socket (`None` after `closeSync`).
type SyncSocket = Option<std::net::TcpStream>;

fn sync_socket<'a>(object: i64) -> Option<&'a mut SyncSocket> {
    let p = native_field(object);
    if p == 0 {
        throw_internal_error("No native peer");
        return None;
    }
    // SAFETY: the box created in `CreateConnectSync`, alive until the object is
    // collected (the finalizer releases it); the isolate is a single thread.
    Some(unsafe { &mut *(p as *mut SyncSocket) })
}

fn finalize_sync_socket(p: usize) {
    // SAFETY: the box from `CreateConnectSync`, released only here.
    drop(unsafe { Box::from_raw(p as *mut SyncSocket) });
}

/// The `OSError` of a `std` system error.
fn io_error(e: &std::io::Error) -> i64 {
    OsError::from_code(e.raw_os_error().unwrap_or(0)).to_dart()
}

/// `[type, text, bytes]` of an IP.
fn dart_ip_address(ip: std::net::IpAddr) -> i64 {
    let (ty, bytes) = match ip {
        std::net::IpAddr::V4(a) => (TYPE_IPV4, a.octets().to_vec()),
        std::net::IpAddr::V6(a) => (TYPE_IPV6, a.octets().to_vec()),
    };
    let text = allocate_str(&ip.to_string());
    with_roots(&[text], || {
        let b = dart_bytes(bytes);
        with_roots(&[b], || dart_fixed_list(&[dart_int(ty), text, b]))
    })
}

/// `SynchronousSocket_LookupRequest(host, type)`: `[[type, text, bytes]…]` or the
/// `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SynchronousSocket_LookupRequest(host: i64, ty: i64) -> i64 {
    let host = String::from_utf8_lossy(&utf8_of_string(host)).into_owned();
    match resolve_name(&host, ty) {
        Ok(v) => {
            let mut items = Vec::with_capacity(v.len());
            let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(v.len().max(1)));
            for (i, e) in v.iter().enumerate() {
                let text = allocate_str(&e.text());
                let item = with_roots(&[text], || {
                    let b = dart_bytes(e.ip());
                    with_roots(&[b], || dart_fixed_list(&[dart_int(e.ty()), text, b]))
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

/// `SynchronousSocket_CreateConnectSync(addr, port)`: `null` when connected, or the
/// `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SynchronousSocket_CreateConnectSync(this: i64, addr: i64, port: i64) -> i64 {
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

/// `SynchronousSocket_Available`: the bytes already received and not read.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SynchronousSocket_Available(this: i64) -> i64 {
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
    // SAFETY: the `TcpStream`'s socket, alive during the call.
    if unsafe { ioctlsocket(s.as_raw_socket() as usize, FIONREAD, &mut n) } != 0 {
        return -1;
    }
    i64::from(n)
}

/// `SynchronousSocket_CloseSync`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SynchronousSocket_CloseSync(this: i64) {
    if let Some(s) = sync_socket(this) {
        s.take();
    }
}

/// `SynchronousSocket_GetPort`: the local port or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SynchronousSocket_GetPort(this: i64) -> i64 {
    let Some(Some(s)) = sync_socket(this) else { return 0 };
    match s.local_addr() {
        Ok(a) => dart_int(i64::from(a.port())),
        Err(e) => io_error(&e),
    }
}

/// `SynchronousSocket_GetRemotePeer`: `[[type, text, bytes], port]` or the
/// `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SynchronousSocket_GetRemotePeer(this: i64) -> i64 {
    let Some(Some(s)) = sync_socket(this) else { return 0 };
    match s.peer_addr() {
        Ok(a) => {
            let e = dart_ip_address(a.ip());
            with_roots(&[e], || dart_fixed_list(&[e, dart_int(i64::from(a.port()))]))
        }
        Err(e) => io_error(&e),
    }
}

/// `SynchronousSocket_Read(len)`: up to `len` bytes (blocks until some arrive);
/// `null` at the end of the stream; or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SynchronousSocket_Read(this: i64, len: i64) -> i64 {
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

/// `SynchronousSocket_ReadList(buffer, offset, bytes)`: writes into `buffer` and
/// returns how many it read (0 at the end of the stream), or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SynchronousSocket_ReadList(this: i64, buffer: i64, start: i64, n: i64) -> i64 {
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

/// `SynchronousSocket_WriteList(buffer, offset, bytes)`: writes everything and
/// returns how many, or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SynchronousSocket_WriteList(this: i64, buffer: i64, start: i64, n: i64) -> i64 {
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
pub extern "C" fn dartforge_native_SynchronousSocket_ShutdownRead(this: i64) {
    if let Some(Some(s)) = sync_socket(this) {
        let _ = s.shutdown(std::net::Shutdown::Read);
    }
}

/// `SynchronousSocket_ShutdownWrite`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SynchronousSocket_ShutdownWrite(this: i64) {
    if let Some(Some(s)) = sync_socket(this) {
        let _ = s.shutdown(std::net::Shutdown::Write);
    }
}

// ---------------------------------------------------------------------------
// Control messages and resources passed through Unix domain sockets
// (`SocketControlMessage`, `ResourceHandle`; N04). The Dart objects come from the
// `_dartforge*` helpers of `native_sdk/io/common_patch.dart`; on Windows, as in the
// VM, there is no descriptor passing.

/// The registered Dart function `name` (from the `dart:io` overlay).
#[cfg(unix)]
fn io_helper(name: &str) -> usize {
    helper(name).unwrap_or_else(|| panic!("compiler bug: dart:io without a registered `{name}`"))
}

/// The `int` of an element of a `List<dynamic>` (scalar or box).
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

/// The `UnsupportedError` of socket-passed resources outside Unix.
#[cfg(windows)]
fn throw_out_of_resources() {
    let msg = allocate_str("This is not supported on this operating system");
    let e = with_roots(&[msg], || dartforge_unsupported_error_new(msg));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

/// `SocketControlMessage.fromHandles(handles)`: the `SCM_RIGHTS` message with the
/// descriptors.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SocketControlMessage_fromHandles(resources: i64) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `(List<ResourceHandle>, int, int) → Object`.
        let f = { let dart_target: usize = io_helper("_dartforgeResourcesMessage"); move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(dart_target, a0, a1, a2) } };
        let (level, ty) = resource_level_and_type();
        f(resources, level, ty)
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
pub extern "C" fn dartforge_native_SocketControlMessageImpl_extractHandles(this: i64) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `(Object, int, int) → List<ResourceHandle>`.
        let f = { let dart_target: usize = io_helper("_dartforgeMessageResources"); move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(dart_target, a0, a1, a2) } };
        let (level, ty) = resource_level_and_type();
        f(this, level, ty)
    }
    #[cfg(windows)]
    {
        let _ = this;
        throw_out_of_resources();
        0
    }
}

/// `Socket_SendMessage(buffer, offset, bytes, controlMessages)`: the bytes sent;
/// `controlMessages` is `[level, type, Uint8List]*`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_SendMessage(this: i64, buffer: i64, start: i64, n: i64, controls: i64) -> i64 {
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
                (Some(level), Some(ty), Some(d)) if data_and_ref => messages.push((level, ty, d)),
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

/// `Socket_ReceiveMessage(len)`: `[level, type, Uint8List]*` and, last, the
/// `Uint8List` of the data.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Socket_ReceiveMessage(this: i64, n: i64) -> i64 {
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
        for (level, ty, d) in messages {
            let b = with_roots(&items, || dart_bytes(d));
            items.extend([dart_int(level), dart_int(ty), b]);
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

/// `ResourceHandleImpl_toFile`: the `RandomAccessFile` over the descriptor (the
/// file becomes its owner).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_ResourceHandleImpl_toFile(this: i64) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `(Object) → int` and `(int) → RandomAccessFile`.
        let descriptor = { let dart_target: usize = io_helper("_dartforgeResourceDescriptor"); move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        let file = { let dart_target: usize = io_helper("_dartforgeResourceFile"); move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        file(NativeFile::new(descriptor(this)))
    }
    #[cfg(windows)]
    {
        let _ = this;
        throw_out_of_resources();
        0
    }
}

/// `ResourceHandleImpl_toRawSocket`: `[type, text, bytes, fd]` of the descriptor's
/// local address.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_ResourceHandleImpl_toRawSocket(this: i64) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `(Object) → int`.
        let descriptor = { let dart_target: usize = io_helper("_dartforgeResourceDescriptor"); move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        let fd = descriptor(this);
        match local_address(fd) {
            Ok(e) => {
                let ty = e.ty();
                let text = e.text();
                let raw = if ty == TYPE_UNIX { text.clone().into_bytes() } else { e.ip() };
                let t = allocate_str(&text);
                let b = with_roots(&[t], || dart_bytes(raw));
                with_roots(&[t, b], || dart_fixed_list(&[dart_int(ty), t, b, dart_int(fd)]))
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

/// `ResourceHandleImpl_toSocket` and `_toRawDatagramSocket`: the VM does not
/// support them (its native returns an `UnsupportedError`, which the typed return
/// turns into a `TypeError`); here the `UnsupportedError` is thrown.
fn throw_unsupported_resource(api: &str) {
    let msg = allocate_str(&format!("ResourceHandle.{api} is not supported"));
    let e = with_roots(&[msg], || dartforge_unsupported_error_new(msg));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_ResourceHandleImpl_toSocket(_this: i64) -> i64 {
    throw_unsupported_resource("toSocket");
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_ResourceHandleImpl_toRawDatagramSocket(_this: i64) -> i64 {
    throw_unsupported_resource("toRawDatagramSocket");
    0
}

#[cfg(all(test, unix))]
mod tests_socket_write_slices {
    use super::*;
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;

    extern "C" fn os_error(message: i64, _code: i64) -> i64 {
        message
    }

    #[test]
    fn reads_full_and_short_buffers_without_retaining_spare_capacity_under_stress() {
        use std::io::Write;
        let (mut sender, receiver) = UnixStream::pair().unwrap();
        receiver.set_nonblocking(true).unwrap();
        let peer = NativeSocket::new(i64::from(receiver.as_raw_fd()));
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let (object, frame) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            let object = h.allocate_instance(1000, 1);
            h.set_field(object, 0, peer, false);
            let frame = h.push_frame_with_slots(1);
            h.set_root(frame, 0, object);
            (object, frame)
        });
        let empty = dartforge_native_Socket_Read(object, 0);
        HEAP.with(|h| assert_eq!(h.borrow().typed_bytes(empty).unwrap(), b""));
        assert_eq!(dartforge_native_Socket_Read(object, 16), 0);
        sender.write_all(b"abcdef").unwrap();
        let first = dartforge_native_Socket_Read(object, 4);
        HEAP.with(|h| assert_eq!(h.borrow().typed_bytes(first).unwrap(), b"abcd"));
        let short = dartforge_native_Socket_Read(object, 65536);
        HEAP.with(|h| {
            let h = h.borrow();
            assert_eq!(h.typed_bytes(short).unwrap(), b"ef");
            assert_eq!(usize::from(h.header(short).n), crate::layout::typed_words(2, 1));
            assert!(h.is_live_object(object));
        });
        assert_eq!(dartforge_native_Socket_Read(object, 16), 0);
        drop(sender);
        assert_eq!(dartforge_native_Socket_Read(object, 16), 0);
        let old_helper = HELPERS.with(|h| h.borrow_mut().insert("_dartforgeOSError".into(), os_error as usize));
        assert_eq!(dartforge_native_Socket_Read(object, -1), 0);
        assert_ne!(dartforge_exception_pending(), 0);
        dartforge_exception_clear();
        let invalid = NativeSocket::new(-1);
        write_native_field(object, invalid);
        assert_eq!(dartforge_native_Socket_Read(object, 8), 0);
        assert_ne!(dartforge_exception_pending(), 0);
        dartforge_exception_clear();
        NativeSocket::free(invalid);
        NativeSocket::free(peer);
        HELPERS.with(|h| {
            let mut h = h.borrow_mut();
            if let Some(previous) = old_helper { h.insert("_dartforgeOSError".into(), previous); }
            else { h.remove("_dartforgeOSError"); }
        });
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.pop_frame(frame);
        });
        HEAP.with(|h| { h.replace(previous); });
    }

    #[test]
    fn writes_internal_external_and_unmodifiable_view_slices() {
        let (sender, mut receiver) = UnixStream::pair().unwrap();
        receiver
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let peer = NativeSocket::new(i64::from(sender.as_raw_fd()));
        let previous = HEAP.with(|h| h.replace(Heap::new(false)));
        let mut external = *b"external";
        let (object, internal, view, native) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            let object = h.allocate_instance(1000, 1);
            h.set_field(object, 0, peer, false);
            let internal = h.new_typed(TYPE_UINT8, 8);
            h.typed_bytes_mut(internal)
                .unwrap()
                .copy_from_slice(b"internal");
            let view = h.new_view(
                crate::typed_lists::view_cid(TYPE_UINT8, true),
                internal,
                2,
                4,
            );
            let native = h.external_typed(TYPE_UINT8, external.as_mut_ptr(), external.len());
            (object, internal, view, native)
        });
        for (buffer, start, count, expected) in [
            (internal, 1, 3, &b"nte"[..]),
            (view, 1, 2, &b"er"[..]),
            (native, 2, 4, &b"tern"[..]),
            (internal, 6, i64::MAX, &b"al"[..]),
        ] {
            assert_eq!(
                dartforge_native_Socket_WriteList(object, buffer, start, count),
                expected.len() as i64
            );
            let mut received = vec![0; expected.len()];
            receiver.read_exact(&mut received).unwrap();
            assert_eq!(received, expected);
        }
        assert_eq!(dartforge_native_Socket_WriteList(object, internal, 8, 3), 0);
        assert_eq!(dartforge_native_Socket_WriteList(object, internal, 0, 0), 0);
        assert_eq!(dartforge_native_Socket_WriteList(object, view, -1, 1), 1);
        let mut received = [0];
        receiver.read_exact(&mut received).unwrap();
        assert_eq!(&received, b"t");
        NativeSocket::free(peer);

        // An OS error constructs a Dart exception after releasing the borrow.
        let old_helper = HELPERS.with(|h| {
            h.borrow_mut()
                .insert("_dartforgeOSError".into(), os_error as usize)
        });
        let invalid = NativeSocket::new(-1);
        write_native_field(object, invalid);
        assert_eq!(dartforge_native_Socket_WriteList(object, internal, 0, 1), 0);
        assert_ne!(dartforge_exception_pending(), 0);
        dartforge_exception_clear();
        NativeSocket::free(invalid);
        HELPERS.with(|h| {
            let mut h = h.borrow_mut();
            if let Some(previous) = old_helper {
                h.insert("_dartforgeOSError".into(), previous);
            } else {
                h.remove("_dartforgeOSError");
            }
        });
        HEAP.with(|h| {
            h.replace(previous);
        });
    }
}
