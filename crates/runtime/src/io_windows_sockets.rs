// Native runtime: the Winsock layer of the `dart:io` sockets (the VM's
// `socket_win.cc` and `socket_base_win.cc`), with the same names as the Unix layer
// (`io_sockets_unix.rs`) that the natives in `io_sockets.rs` call. The
// "descriptor" is the [`Handle`] of `io_windows_events.rs`.
//
// Windows has no Unix domain sockets in `dart:io` (the VM answers with an
// `OSError`); name resolution uses the Unicode functions (`GetAddrInfoW`,
// `GetNameInfoW`).

#[cfg(windows)]
mod windows_only {
    use super::*;
    use std::sync::Arc;

    const SO_REUSEADDR: i32 = 4;
    /// `SO_EXCLUSIVEADDRUSE` (`~SO_REUSEADDR`).
    const SO_EXCLUSIVEADDRUSE: i32 = !SO_REUSEADDR;
    const SO_LINGER: i32 = 0x80;
    const SO_BROADCAST: i32 = 0x20;
    const TCP_NODELAY: i32 = 1;
    const IPV6_V6ONLY: i32 = 27;
    const IP_MULTICAST_IF: i32 = 9;
    const IP_MULTICAST_TTL: i32 = 10;
    const IP_MULTICAST_LOOP: i32 = 11;
    const IPV6_MULTICAST_IF: i32 = 9;
    const IPV6_MULTICAST_HOPS: i32 = 10;
    const IPV6_MULTICAST_LOOP: i32 = 11;
    const SOMAXCONN: i32 = 0x7fff_ffff;
    const AI_ADDRCONFIG: i32 = 0x400;
    const NI_NUMERICHOST: i32 = 0x2;
    const NI_NAMEREQD: i32 = 0x4;
    const WSAEINPROGRESS: i64 = 10036;
    const WSAEINVAL: i64 = 10022;
    const WSAEADDRINUSE: i64 = 10048;
    const WSAEADDRNOTAVAIL: i64 = 10049;
    const ERROR_BUFFER_OVERFLOW: u32 = 111;
    const FILE_TYPE_DISK: u32 = 1;
    const FILE_TYPE_CHAR: u32 = 2;
    const FILE_TYPE_PIPE: u32 = 3;

    /// `ADDRINFOW` (on Windows, `ai_canonname` comes before `ai_addr`).
    #[repr(C)]
    struct WinAddrInfo {
        ai_flags: i32,
        ai_family: i32,
        ai_socktype: i32,
        ai_protocol: i32,
        ai_addrlen: usize,
        ai_canonname: *mut u16,
        ai_addr: *const u8,
        ai_next: *mut WinAddrInfo,
    }

    /// The beginning of `IP_ADAPTER_ADDRESSES_LH` (up to `Ipv6IfIndex`).
    #[repr(C)]
    struct Adapter {
        size: u32,
        ix: u32,
        next_offset: *const Adapter,
        adapter_name: *const u8,
        first_unicast: *const UnicastAddress,
        first_anycast: *const u8,
        first_multicast: *const u8,
        first_dns: *const u8,
        dns_suffix: *const u16,
        description: *const u16,
        friendly_name: *const u16,
        physical_address: [u8; 8],
        physical_address_size: u32,
        flag_bits: u32,
        mtu: u32,
        ty: u32,
        operational_state: u32,
        ipv6_index: u32,
    }

    /// The beginning of `IP_ADAPTER_UNICAST_ADDRESS_LH`.
    #[repr(C)]
    struct UnicastAddress {
        size: u32,
        flag_bits: u32,
        next_offset: *const UnicastAddress,
        address: *const u8,
        address_size: i32,
    }

    #[link(name = "ws2_32")]
    unsafe extern "system" {
        fn GetAddrInfoW(no: *const u16, service: *const u16, hints: *const WinAddrInfo, res: *mut *mut WinAddrInfo) -> i32;
        fn FreeAddrInfoW(res: *mut WinAddrInfo);
        fn GetNameInfoW(e: *const u8, n: i32, host: *mut u16, hn: u32, serv: *mut u16, sn: u32, flag_bits: i32) -> i32;
        fn InetPtonW(family: i32, text: *const u16, dest: *mut u8) -> i32;
    }

    #[link(name = "iphlpapi")]
    unsafe extern "system" {
        fn GetAdaptersAddresses(family: u32, flag_bits: u32, reserved: *mut std::ffi::c_void, addresses: *mut u8, size: *mut u32) -> u32;
    }

    /// A NUL-terminated UTF-16 string.
    fn wide(t: &str) -> Vec<u16> {
        t.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The text of a NUL-terminated UTF-16 buffer.
    fn from_wide(b: &[u16]) -> String {
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        String::from_utf16_lossy(&b[..end])
    }

    /// The text of a NUL-terminated UTF-16 pointer.
    ///
    /// # Safety
    /// `p` is null or points to a NUL-terminated string.
    unsafe fn from_wide_pointer(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut n = 0;
        // SAFETY: guaranteed by the caller.
        while unsafe { *p.add(n) } != 0 {
            n += 1;
        }
        // SAFETY: the `n` characters before the NUL.
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, n) })
    }

    // -----------------------------------------------------------------------
    // Addresses (`RawAddr`): Winsock's `sockaddr_in`/`sockaddr_in6`, with the
    // same layout as Linux's.

    impl OsAddress {
        pub(super) fn empty() -> OsAddress {
            OsAddress { bytes: [0; 128], size: 128 }
        }

        pub(super) fn family(&self) -> u16 {
            u16::from_le_bytes([self.bytes[0], self.bytes[1]])
        }

        fn write_family(&mut self, f: u16, size: u32) {
            self.bytes[..2].copy_from_slice(&f.to_le_bytes());
            self.size = size;
        }

        pub(super) fn from_ip(ip: &[u8]) -> Option<OsAddress> {
            let mut e = OsAddress::empty();
            match ip.len() {
                4 => {
                    e.write_family(AF_INET, 16);
                    e.bytes[4..8].copy_from_slice(ip);
                }
                16 => {
                    e.write_family(AF_INET6, 28);
                    e.bytes[8..24].copy_from_slice(ip);
                }
                _ => return None,
            }
            Some(e)
        }

        /// Unix domain sockets do not exist in Windows `dart:io`.
        pub(super) fn from_unix_path(_path: &[u8]) -> IoResult<OsAddress> {
            Err(without_unix_domain())
        }

        pub(super) fn from_pointer(p: *const u8, n: u32) -> OsAddress {
            let mut e = OsAddress::empty();
            let n = (n as usize).min(128);
            // SAFETY: `p` points to a system `sockaddr` of `n` bytes.
            e.bytes[..n].copy_from_slice(unsafe { std::slice::from_raw_parts(p, n) });
            e.size = n as u32;
            e
        }

        pub(super) fn is_ipv6(&self) -> bool {
            self.family() == AF_INET6
        }

        pub(super) fn ty(&self) -> i64 {
            if self.is_ipv6() { TYPE_IPV6 } else { TYPE_IPV4 }
        }

        pub(super) fn port(&self) -> i64 {
            i64::from(u16::from_be_bytes([self.bytes[2], self.bytes[3]]))
        }

        pub(super) fn write_port(&mut self, p: i64) {
            self.bytes[2..4].copy_from_slice(&(p as u16).to_be_bytes());
        }

        pub(super) fn scope(&self) -> i64 {
            if self.is_ipv6() { i64::from(u32::from_le_bytes(self.bytes[24..28].try_into().expect("4 bytes"))) } else { 0 }
        }

        pub(super) fn write_scope(&mut self, s: i64) {
            if self.is_ipv6() {
                self.bytes[24..28].copy_from_slice(&(s as u32).to_le_bytes());
            }
        }

        pub(super) fn ip(&self) -> Vec<u8> {
            if self.is_ipv6() { self.bytes[8..24].to_vec() } else { self.bytes[4..8].to_vec() }
        }

        pub(super) fn family_size(&self) -> u32 {
            if self.is_ipv6() { 28 } else { 16 }
        }

        /// The numeric text, with `%interface` for link-local scoped IPv6.
        pub(super) fn text(&self) -> String {
            start_winsock();
            let mut b = [0u16; 64];
            // SAFETY: the buffer has 64 units (INET6_ADDRSTRLEN with scope).
            let r = unsafe {
                GetNameInfoW(self.bytes.as_ptr(), self.family_size() as i32, b.as_mut_ptr(), b.len() as u32, std::ptr::null_mut(), 0, NI_NUMERICHOST)
            };
            if r != 0 { String::new() } else { from_wide(&b) }
        }
    }

    fn without_unix_domain() -> OsError {
        OsError { code: -1, message: "Unix domain sockets are not available on this operating system.".to_string() }
    }

    // -----------------------------------------------------------------------
    // Operations.

    pub(super) fn last_error() -> OsError {
        OsError::from_code(unsafe { WSAGetLastError() })
    }

    /// Closes `s` preserving the current error.
    fn close_with_error(s: RawHandle) -> OsError {
        let e = last_error();
        // SAFETY: the socket belongs to the caller and was not handed to anyone.
        unsafe { closesocket(s) };
        e
    }

    fn set_int_option(s: RawHandle, level: i32, option: i32, value: i32) -> bool {
        // SAFETY: `value` lives during the call.
        unsafe { setsockopt(s, level, option, (&value as *const i32).cast(), 4) == 0 }
    }

    /// An integer option; the multicast ones may come in one byte.
    fn get_int_option(s: RawHandle, level: i32, option: i32) -> Option<i64> {
        let mut v = [0u8; 4];
        let mut n = 4i32;
        // SAFETY: `v` has 4 bytes; the system says how many it used.
        if unsafe { getsockopt(s, level, option, v.as_mut_ptr(), &mut n) } != 0 {
            return None;
        }
        Some(if n == 1 { i64::from(v[0]) } else { i64::from(i32::from_le_bytes(v)) })
    }

    /// The handle of a socket descriptor (the error, if already closed).
    fn sock(fd: i64) -> IoResult<Arc<Handle>> {
        handle_of(fd).ok_or_else(invalid_handle)
    }

    /// The `SOCKET` of a descriptor, with the error in `WSAGetLastError` if the
    /// socket was already closed (the natives read the error from there).
    fn raw_socket(fd: i64) -> Option<RawHandle> {
        match sock(fd) {
            Ok(m) => Some(m.raw),
            Err(_) => {
                // SAFETY: only writes the thread's error.
                unsafe { WSASetLastError(6) };
                None
            }
        }
    }

    /// `Socket::CreateConnect` and `CreateBindConnect`: `SO_LINGER` of 10 s, the
    /// `bind` to the source and `ConnectEx`, which completes through the port.
    fn connect_socket(dest: &OsAddress, origin: &OsAddress) -> IoResult<i64> {
        let s = new_socket(dest.family(), SOCK_STREAM, 0)?;
        let wait_entry: [u16; 2] = [1, 10];
        // SAFETY: `linger` has two `u_short`s.
        if unsafe { setsockopt(s, SOL_SOCKET, SO_LINGER, wait_entry.as_ptr().cast(), 4) } != 0 {
            return Err(close_with_error(s));
        }
        let m = Handle::new(HandleKind::Client, s);
        // SAFETY: `origin` is a valid `sockaddr`.
        if unsafe { bind(s, origin.bytes.as_ptr(), origin.family_size() as i32) } != 0 {
            let e = last_error();
            discard(&m);
            return Err(e);
        }
        if let Err(e) = start_connect(&m, dest) {
            discard(&m);
            return Err(e);
        }
        Ok(descriptor_of(m))
    }

    pub(super) fn create_connection(e: &OsAddress) -> IoResult<i64> {
        let mut origin = OsAddress::empty();
        origin.write_family(e.family(), e.family_size());
        connect_socket(e, &origin)
    }

    pub(super) fn create_connection_with_source(e: &OsAddress, origin: &OsAddress) -> IoResult<i64> {
        connect_socket(e, origin)
    }

    pub(super) fn create_unix_connection(_e: &OsAddress, _source: Option<&OsAddress>) -> IoResult<i64> {
        Err(without_unix_domain())
    }

    /// `ServerSocket::CreateBindListen`: `SO_EXCLUSIVEADDRUSE`, `IPV6_V6ONLY`,
    /// port 65535 avoided and the `AcceptEx`s issued.
    pub(super) fn create_listener(e: &OsAddress, queue: i64, v6_only: bool) -> IoResult<i64> {
        let s = new_socket(e.family(), SOCK_STREAM, IPPROTO_TCP)?;
        if !set_int_option(s, SOL_SOCKET, SO_EXCLUSIVEADDRUSE, 1) {
            return Err(close_with_error(s));
        }
        if e.is_ipv6() {
            set_int_option(s, IPPROTO_IPV6, IPV6_V6ONLY, i32::from(v6_only));
        }
        // SAFETY: `e` is a valid `sockaddr`.
        if unsafe { bind(s, e.bytes.as_ptr(), e.family_size() as i32) } != 0 {
            return Err(close_with_error(s));
        }
        if e.port() == 0 && socket_port(s) == 65535 {
            // Another socket before releasing this one guarantees another port.
            let new = create_listener(e, queue, v6_only);
            // SAFETY: the socket was not handed to anyone.
            unsafe { closesocket(s) };
            return new;
        }
        // SAFETY: open socket.
        if unsafe { listen(s, if queue > 0 { queue as i32 } else { SOMAXCONN }) } != 0 {
            return Err(close_with_error(s));
        }
        let m = Handle::new(HandleKind::Listen, s);
        m.state().family = e.family();
        if !start_accepting(&m) {
            discard(&m);
            return Err(OsError { code: -1, message: "Failed to start accept".to_string() });
        }
        Ok(descriptor_of(m))
    }

    pub(super) fn create_unix_listener(_e: &OsAddress, _queue: i64) -> IoResult<i64> {
        Err(without_unix_domain())
    }

    /// `Socket::CreateBindDatagram`.
    pub(super) fn create_datagram(e: &OsAddress, reuse_address: bool, reuse_port: bool, ttl: i64) -> IoResult<i64> {
        let s = new_socket(e.family(), SOCK_DGRAM, IPPROTO_UDP)?;
        if reuse_address && !set_int_option(s, SOL_SOCKET, SO_REUSEADDR, 1) {
            return Err(close_with_error(s));
        }
        if reuse_port {
            eprintln!("Dart Socket ERROR: `reusePort` not supported for Windows.");
        }
        let (level, name) = if e.is_ipv6() { (IPPROTO_IPV6, IPV6_MULTICAST_HOPS) } else { (IPPROTO_IP, IP_MULTICAST_TTL) };
        if !set_int_option(s, level, name, ttl as i32) {
            return Err(close_with_error(s));
        }
        // SAFETY: `e` is a valid `sockaddr`.
        if unsafe { bind(s, e.bytes.as_ptr(), e.family_size() as i32) } != 0 {
            return Err(close_with_error(s));
        }
        Ok(descriptor_of(Handle::new(HandleKind::Datagram, s)))
    }

    fn socket_port(s: RawHandle) -> i64 {
        let mut e = OsAddress::empty();
        let mut n = 128i32;
        // SAFETY: `e` has 128 bytes.
        if unsafe { getsockname(s, e.bytes.as_mut_ptr(), &mut n) } != 0 {
            return 0;
        }
        e.port()
    }

    /// `SocketBase::GetPort` (0 on error).
    pub(super) fn local_port(fd: i64) -> i64 {
        raw_socket(fd).map_or(0, socket_port)
    }

    /// `SocketBase::GetRemotePeer`: the address that `AcceptEx` gave, or
    /// `getpeername`.
    pub(super) fn remote_peer(fd: i64) -> IoResult<(OsAddress, i64)> {
        let m = sock(fd)?;
        if m.ty == HandleKind::Client {
            if let Some(e) = m.state().remote.clone() {
                let port = e.port();
                return Ok((e, port));
            }
        }
        let mut e = OsAddress::empty();
        let mut n = 128i32;
        // SAFETY: `e` has 128 bytes.
        if unsafe { getpeername(m.raw, e.bytes.as_mut_ptr(), &mut n) } != 0 {
            return Err(last_error());
        }
        e.size = n as u32;
        let port = e.port();
        Ok((e, port))
    }

    /// `SocketBase::GetError`: the handle's last error.
    pub(super) fn pending_error(fd: i64) -> i32 {
        sock(fd).map_or(6, |m| m.state().last_error as i32)
    }

    /// `SocketBase::GetType` (`GetFileType`).
    pub(super) fn descriptor_type(fd: i64) -> IoResult<i64> {
        let m = sock(fd)?;
        // SAFETY: queries the type of an open handle.
        Ok(match unsafe { GetFileType(m.raw) } {
            FILE_TYPE_CHAR => 0,
            FILE_TYPE_PIPE => 1,
            FILE_TYPE_DISK => 2,
            _ => match unsafe { GetLastError() } {
                0 => 4,
                c => return Err(OsError::from_code(c as i32)),
            },
        })
    }

    pub(super) fn in_progress_code() -> i64 {
        WSAEINPROGRESS
    }

    pub(super) fn is_bind_error(error: i64) -> bool {
        [WSAEADDRINUSE, WSAEADDRNOTAVAIL, WSAEINVAL].contains(&error)
    }

    /// `Socket_GetOption`: 0 `TCP_NODELAY`, 1 multicast loop, 2 multicast TTL,
    /// 4 `SO_BROADCAST`.
    pub(super) fn get_option(fd: i64, option: i64, v4: bool) -> Option<i64> {
        let s = raw_socket(fd)?;
        match option {
            0 => get_int_option(s, IPPROTO_TCP, TCP_NODELAY),
            1 => {
                let (level, name) = if v4 { (IPPROTO_IP, IP_MULTICAST_LOOP) } else { (IPPROTO_IPV6, IPV6_MULTICAST_LOOP) };
                get_int_option(s, level, name)
            }
            2 => {
                let (level, name) = if v4 { (IPPROTO_IP, IP_MULTICAST_TTL) } else { (IPPROTO_IPV6, IPV6_MULTICAST_HOPS) };
                get_int_option(s, level, name)
            }
            4 => get_int_option(s, SOL_SOCKET, SO_BROADCAST),
            _ => None,
        }
    }

    pub(super) fn set_option(fd: i64, option: i64, v4: bool, value: i64) -> bool {
        let (level, name) = match option {
            0 => (IPPROTO_TCP, TCP_NODELAY),
            1 if v4 => (IPPROTO_IP, IP_MULTICAST_LOOP),
            1 => (IPPROTO_IPV6, IPV6_MULTICAST_LOOP),
            2 if v4 => (IPPROTO_IP, IP_MULTICAST_TTL),
            2 => (IPPROTO_IPV6, IPV6_MULTICAST_HOPS),
            4 => (SOL_SOCKET, SO_BROADCAST),
            _ => return false,
        };
        raw_socket(fd).is_some_and(|s| set_int_option(s, level, name, value as i32))
    }

    pub(super) fn set_raw_option(fd: i64, level: i64, option: i64, data: &[u8]) -> bool {
        let Some(s) = raw_socket(fd) else { return false };
        // SAFETY: `data` lives during the call.
        unsafe { setsockopt(s, level as i32, option as i32, data.as_ptr(), data.len() as i32) == 0 }
    }

    pub(super) fn get_raw_option(fd: i64, level: i64, option: i64, data: &mut Vec<u8>) -> bool {
        let Some(s) = raw_socket(fd) else { return false };
        let mut n = data.len() as i32;
        // SAFETY: `data` is writable with `n` bytes.
        let ok = unsafe { getsockopt(s, level as i32, option as i32, data.as_mut_ptr(), &mut n) } == 0;
        data.truncate(n.max(0) as usize);
        ok
    }

    pub(super) fn raw_option_constant(key: i64) -> Option<i64> {
        Some(i64::from(match key {
            0 => SOL_SOCKET,
            1 => IPPROTO_IP,
            2 => IP_MULTICAST_IF,
            3 => IPPROTO_IPV6,
            4 => IPV6_MULTICAST_IF,
            5 => IPPROTO_TCP,
            6 => IPPROTO_UDP,
            _ => return None,
        }))
    }

    // -----------------------------------------------------------------------
    // Names and interfaces.

    /// `SocketBase::LookupAddress` (first with `AI_ADDRCONFIG`).
    pub(super) fn resolve_name(host: &str, ty: i64) -> IoResult<Vec<OsAddress>> {
        start_winsock();
        let name = wide(host);
        let family = match ty {
            TYPE_IPV4 => i32::from(AF_INET),
            TYPE_IPV6 => i32::from(AF_INET6),
            _ => 0,
        };
        let mut hints = WinAddrInfo {
            ai_flags: AI_ADDRCONFIG,
            ai_family: family,
            ai_socktype: SOCK_STREAM,
            ai_protocol: IPPROTO_TCP,
            ai_addrlen: 0,
            ai_canonname: std::ptr::null_mut(),
            ai_addr: std::ptr::null(),
            ai_next: std::ptr::null_mut(),
        };
        let mut res: *mut WinAddrInfo = std::ptr::null_mut();
        // SAFETY: `name`, `hints` and `res` live during the calls.
        let mut status = unsafe { GetAddrInfoW(name.as_ptr(), std::ptr::null(), &hints, &mut res) };
        if status != 0 {
            hints.ai_flags = 0;
            status = unsafe { GetAddrInfoW(name.as_ptr(), std::ptr::null(), &hints, &mut res) };
            if status != 0 {
                return Err(OsError::from_code(status));
            }
        }
        let mut out = Vec::new();
        let mut current = res;
        while !current.is_null() {
            // SAFETY: node of the list returned by `GetAddrInfoW`.
            let i = unsafe { &*current };
            if i.ai_family == i32::from(AF_INET) || i.ai_family == i32::from(AF_INET6) {
                out.push(OsAddress::from_pointer(i.ai_addr, i.ai_addrlen as u32));
            }
            current = i.ai_next;
        }
        // SAFETY: the list came from `GetAddrInfoW`.
        unsafe { FreeAddrInfoW(res) };
        Ok(out)
    }

    /// `SocketBase::ReverseLookup`.
    pub(super) fn address_name(e: &OsAddress) -> IoResult<String> {
        start_winsock();
        let mut b = [0u16; 1025];
        // SAFETY: the buffer has `NI_MAXHOST` units.
        let status = unsafe {
            GetNameInfoW(e.bytes.as_ptr(), e.family_size() as i32, b.as_mut_ptr(), b.len() as u32, std::ptr::null_mut(), 0, NI_NAMEREQD)
        };
        if status != 0 {
            return Err(OsError::from_code(status));
        }
        Ok(from_wide(&b))
    }

    /// `SocketBase::ListInterfaces` (`GetAdaptersAddresses`, without anycast,
    /// multicast and DNS): (address, friendly name, index).
    pub(super) fn list_interfaces(ty: i64) -> IoResult<Vec<(OsAddress, String, i64)>> {
        start_winsock();
        const GAA_FLAG_SKIP_ANYCAST: u32 = 0x2;
        const GAA_FLAG_SKIP_MULTICAST: u32 = 0x4;
        const GAA_FLAG_SKIP_DNS_SERVER: u32 = 0x8;
        let family = match ty {
            TYPE_IPV4 => u32::from(AF_INET),
            TYPE_IPV6 => u32::from(AF_INET6),
            _ => 0,
        };
        let flag_bits = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
        // The size changes between calls if an interface appears: repeat while
        // space is missing.
        let mut size = 16 * 1024u32;
        let mut buf: Vec<u64>;
        loop {
            buf = vec![0; (size as usize).div_ceil(8)];
            // SAFETY: the buffer has `size` bytes, aligned to 8.
            let r = unsafe { GetAdaptersAddresses(family, flag_bits, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size) };
            match r {
                0 => break,
                ERROR_BUFFER_OVERFLOW => continue,
                c => return Err(OsError::from_code(c as i32)),
            }
        }
        let mut out = Vec::new();
        let mut a = buf.as_ptr().cast::<Adapter>();
        while !a.is_null() {
            // SAFETY: node of the list the system wrote into the buffer.
            let ad = unsafe { &*a };
            // SAFETY: the friendly name is a NUL-terminated string.
            let name = unsafe { from_wide_pointer(ad.friendly_name) };
            let ix = if ad.ipv6_index != 0 { ad.ipv6_index } else { ad.ix };
            let mut u = ad.first_unicast;
            while !u.is_null() {
                // SAFETY: node of the adapter's address list.
                let un = unsafe { &*u };
                out.push((OsAddress::from_pointer(un.address, un.address_size.max(0) as u32), name.clone(), i64::from(ix)));
                u = un.next_offset;
            }
            a = ad.next_offset;
        }
        Ok(out)
    }

    /// `SocketBase::ParseAddress` (`InetPtonW`).
    pub(super) fn parse_address(text: &str) -> Option<Vec<u8>> {
        start_winsock();
        let v6 = text.contains(':');
        let mut b = [0u8; 16];
        let family = if v6 { AF_INET6 } else { AF_INET };
        let t = wide(text);
        // SAFETY: `b` has 16 bytes.
        (unsafe { InetPtonW(i32::from(family), t.as_ptr(), b.as_mut_ptr()) } == 1).then(|| b[..if v6 { 16 } else { 4 }].to_vec())
    }
}

#[cfg(windows)]
use windows_only::*;
