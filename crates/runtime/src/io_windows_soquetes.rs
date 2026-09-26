// Runtime nativo: a camada Winsock dos soquetes do `dart:io`
// (`socket_win.cc` e `socket_base_win.cc` da VM), com os mesmos nomes da
// camada Unix (`io_soquetes_unix.rs`) que os natives de `io_soquetes.rs`
// chamam. O "descritor" é o [`Manipulador`] de `io_windows_eventos.rs`.
//
// O Windows não tem soquetes de domínio Unix no `dart:io` (a VM responde
// com um `OSError`); a resolução de nomes usa as funções Unicode
// (`GetAddrInfoW`, `GetNameInfoW`).

#[cfg(windows)]
mod so_windows {
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

    /// `ADDRINFOW` (no Windows, `ai_canonname` vem antes de `ai_addr`).
    #[repr(C)]
    struct InfoDeEndereco {
        ai_flags: i32,
        ai_family: i32,
        ai_socktype: i32,
        ai_protocol: i32,
        ai_addrlen: usize,
        ai_canonname: *mut u16,
        ai_addr: *const u8,
        ai_next: *mut InfoDeEndereco,
    }

    /// O começo de `IP_ADAPTER_ADDRESSES_LH` (até `Ipv6IfIndex`).
    #[repr(C)]
    struct Adaptador {
        tamanho: u32,
        indice: u32,
        proximo: *const Adaptador,
        nome_do_adaptador: *const u8,
        primeiro_unicast: *const EnderecoUnicast,
        primeiro_anycast: *const u8,
        primeiro_multicast: *const u8,
        primeiro_dns: *const u8,
        sufixo_dns: *const u16,
        descricao: *const u16,
        nome_amigavel: *const u16,
        endereco_fisico: [u8; 8],
        tamanho_do_endereco_fisico: u32,
        bandeiras: u32,
        mtu: u32,
        tipo: u32,
        estado_operacional: u32,
        indice_ipv6: u32,
    }

    /// O começo de `IP_ADAPTER_UNICAST_ADDRESS_LH`.
    #[repr(C)]
    struct EnderecoUnicast {
        tamanho: u32,
        bandeiras: u32,
        proximo: *const EnderecoUnicast,
        endereco: *const u8,
        tamanho_do_endereco: i32,
    }

    #[link(name = "ws2_32")]
    unsafe extern "system" {
        fn GetAddrInfoW(no: *const u16, servico: *const u16, dicas: *const InfoDeEndereco, res: *mut *mut InfoDeEndereco) -> i32;
        fn FreeAddrInfoW(res: *mut InfoDeEndereco);
        fn GetNameInfoW(e: *const u8, n: i32, host: *mut u16, hn: u32, serv: *mut u16, sn: u32, bandeiras: i32) -> i32;
        fn InetPtonW(familia: i32, texto: *const u16, destino: *mut u8) -> i32;
    }

    #[link(name = "iphlpapi")]
    unsafe extern "system" {
        fn GetAdaptersAddresses(familia: u32, bandeiras: u32, reservado: *mut std::ffi::c_void, enderecos: *mut u8, tamanho: *mut u32) -> u32;
    }

    /// Um texto terminado em NUL em UTF-16.
    fn largo(t: &str) -> Vec<u16> {
        t.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// O texto de um buffer UTF-16 terminado em NUL.
    fn de_largo(b: &[u16]) -> String {
        let fim = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        String::from_utf16_lossy(&b[..fim])
    }

    /// O texto de um ponteiro UTF-16 terminado em NUL.
    ///
    /// # Safety
    /// `p` é nulo ou aponta para um texto terminado em NUL.
    unsafe fn de_ponteiro_largo(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut n = 0;
        // SAFETY: garantido por quem chama.
        while unsafe { *p.add(n) } != 0 {
            n += 1;
        }
        // SAFETY: os `n` caracteres antes do NUL.
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, n) })
    }

    // -----------------------------------------------------------------------
    // Endereços (`RawAddr`): `sockaddr_in`/`sockaddr_in6` do Winsock, com a
    // mesma disposição dos do Linux.

    impl EnderecoSo {
        pub(super) fn vazio() -> EnderecoSo {
            EnderecoSo { bytes: [0; 128], tamanho: 128 }
        }

        pub(super) fn familia(&self) -> u16 {
            u16::from_le_bytes([self.bytes[0], self.bytes[1]])
        }

        fn gravar_familia(&mut self, f: u16, tamanho: u32) {
            self.bytes[..2].copy_from_slice(&f.to_le_bytes());
            self.tamanho = tamanho;
        }

        pub(super) fn de_ip(ip: &[u8]) -> Option<EnderecoSo> {
            let mut e = EnderecoSo::vazio();
            match ip.len() {
                4 => {
                    e.gravar_familia(AF_INET, 16);
                    e.bytes[4..8].copy_from_slice(ip);
                }
                16 => {
                    e.gravar_familia(AF_INET6, 28);
                    e.bytes[8..24].copy_from_slice(ip);
                }
                _ => return None,
            }
            Some(e)
        }

        /// Os soquetes de domínio Unix não existem no `dart:io` do Windows.
        pub(super) fn de_caminho_unix(_caminho: &[u8]) -> ResultadoIo<EnderecoSo> {
            Err(sem_dominio_unix())
        }

        pub(super) fn de_ponteiro(p: *const u8, n: u32) -> EnderecoSo {
            let mut e = EnderecoSo::vazio();
            let n = (n as usize).min(128);
            // SAFETY: `p` aponta para um `sockaddr` de `n` bytes do sistema.
            e.bytes[..n].copy_from_slice(unsafe { std::slice::from_raw_parts(p, n) });
            e.tamanho = n as u32;
            e
        }

        pub(super) fn e_ipv6(&self) -> bool {
            self.familia() == AF_INET6
        }

        pub(super) fn tipo(&self) -> i64 {
            if self.e_ipv6() { TIPO_IPV6 } else { TIPO_IPV4 }
        }

        pub(super) fn porta(&self) -> i64 {
            i64::from(u16::from_be_bytes([self.bytes[2], self.bytes[3]]))
        }

        pub(super) fn gravar_porta(&mut self, p: i64) {
            self.bytes[2..4].copy_from_slice(&(p as u16).to_be_bytes());
        }

        pub(super) fn escopo(&self) -> i64 {
            if self.e_ipv6() { i64::from(u32::from_le_bytes(self.bytes[24..28].try_into().expect("4 bytes"))) } else { 0 }
        }

        pub(super) fn gravar_escopo(&mut self, s: i64) {
            if self.e_ipv6() {
                self.bytes[24..28].copy_from_slice(&(s as u32).to_le_bytes());
            }
        }

        pub(super) fn ip(&self) -> Vec<u8> {
            if self.e_ipv6() { self.bytes[8..24].to_vec() } else { self.bytes[4..8].to_vec() }
        }

        pub(super) fn tamanho_da_familia(&self) -> u32 {
            if self.e_ipv6() { 28 } else { 16 }
        }

        /// O texto numérico, com `%interface` no IPv6 de escopo local.
        pub(super) fn texto(&self) -> String {
            iniciar_winsock();
            let mut b = [0u16; 64];
            // SAFETY: o buffer tem 64 unidades (INET6_ADDRSTRLEN com escopo).
            let r = unsafe {
                GetNameInfoW(self.bytes.as_ptr(), self.tamanho_da_familia() as i32, b.as_mut_ptr(), b.len() as u32, std::ptr::null_mut(), 0, NI_NUMERICHOST)
            };
            if r != 0 { String::new() } else { de_largo(&b) }
        }
    }

    fn sem_dominio_unix() -> ErroDoSo {
        ErroDoSo { codigo: -1, mensagem: "Unix domain sockets are not available on this operating system.".to_string() }
    }

    // -----------------------------------------------------------------------
    // Operações.

    pub(super) fn ultimo_erro() -> ErroDoSo {
        ErroDoSo::do_codigo(unsafe { WSAGetLastError() })
    }

    /// Fecha `s` preservando o erro corrente.
    fn fechar_com_erro(s: Bruto) -> ErroDoSo {
        let e = ultimo_erro();
        // SAFETY: o soquete é de quem chama e não foi entregue a ninguém.
        unsafe { closesocket(s) };
        e
    }

    fn definir_opcao_int(s: Bruto, nivel: i32, opcao: i32, valor: i32) -> bool {
        // SAFETY: `valor` vive durante a chamada.
        unsafe { setsockopt(s, nivel, opcao, (&valor as *const i32).cast(), 4) == 0 }
    }

    /// Uma opção inteira; as de multicast podem vir num byte.
    fn ler_opcao_int(s: Bruto, nivel: i32, opcao: i32) -> Option<i64> {
        let mut v = [0u8; 4];
        let mut n = 4i32;
        // SAFETY: `v` tem 4 bytes; o sistema diz quantos usou.
        if unsafe { getsockopt(s, nivel, opcao, v.as_mut_ptr(), &mut n) } != 0 {
            return None;
        }
        Some(if n == 1 { i64::from(v[0]) } else { i64::from(i32::from_le_bytes(v)) })
    }

    /// O manipulador de um descritor de soquete (o erro, se já fechado).
    fn soquete(fd: i64) -> ResultadoIo<Arc<Manipulador>> {
        manipulador_de(fd).ok_or_else(handle_invalido)
    }

    /// O `SOCKET` de um descritor, com o erro no `WSAGetLastError` se o
    /// soquete já foi fechado (os natives leem o erro de lá).
    fn soquete_bruto(fd: i64) -> Option<Bruto> {
        match soquete(fd) {
            Ok(m) => Some(m.bruto),
            Err(_) => {
                // SAFETY: só grava o erro da thread.
                unsafe { WSASetLastError(6) };
                None
            }
        }
    }

    /// `Socket::CreateConnect` e `CreateBindConnect`: `SO_LINGER` de 10 s,
    /// o `bind` na origem e o `ConnectEx`, que conclui pela porta.
    fn conectar(destino: &EnderecoSo, origem: &EnderecoSo) -> ResultadoIo<i64> {
        let s = novo_soquete(destino.familia(), SOCK_STREAM, 0)?;
        let espera: [u16; 2] = [1, 10];
        // SAFETY: `linger` tem dois `u_short`.
        if unsafe { setsockopt(s, SOL_SOCKET, SO_LINGER, espera.as_ptr().cast(), 4) } != 0 {
            return Err(fechar_com_erro(s));
        }
        let m = Manipulador::novo(TipoDeManipulador::Cliente, s);
        // SAFETY: `origem` é um `sockaddr` válido.
        if unsafe { bind(s, origem.bytes.as_ptr(), origem.tamanho_da_familia() as i32) } != 0 {
            let e = ultimo_erro();
            descartar(&m);
            return Err(e);
        }
        if let Err(e) = iniciar_conexao(&m, destino) {
            descartar(&m);
            return Err(e);
        }
        Ok(descritor_de(m))
    }

    pub(super) fn criar_conexao(e: &EnderecoSo) -> ResultadoIo<i64> {
        let mut origem = EnderecoSo::vazio();
        origem.gravar_familia(e.familia(), e.tamanho_da_familia());
        conectar(e, &origem)
    }

    pub(super) fn criar_conexao_com_origem(e: &EnderecoSo, origem: &EnderecoSo) -> ResultadoIo<i64> {
        conectar(e, origem)
    }

    pub(super) fn criar_conexao_unix(_e: &EnderecoSo, _origem: Option<&EnderecoSo>) -> ResultadoIo<i64> {
        Err(sem_dominio_unix())
    }

    /// `ServerSocket::CreateBindListen`: `SO_EXCLUSIVEADDRUSE`,
    /// `IPV6_V6ONLY`, a porta 65535 evitada e os `AcceptEx` emitidos.
    pub(super) fn criar_escuta(e: &EnderecoSo, fila: i64, so_v6: bool) -> ResultadoIo<i64> {
        let s = novo_soquete(e.familia(), SOCK_STREAM, IPPROTO_TCP)?;
        if !definir_opcao_int(s, SOL_SOCKET, SO_EXCLUSIVEADDRUSE, 1) {
            return Err(fechar_com_erro(s));
        }
        if e.e_ipv6() {
            definir_opcao_int(s, IPPROTO_IPV6, IPV6_V6ONLY, i32::from(so_v6));
        }
        // SAFETY: `e` é um `sockaddr` válido.
        if unsafe { bind(s, e.bytes.as_ptr(), e.tamanho_da_familia() as i32) } != 0 {
            return Err(fechar_com_erro(s));
        }
        if e.porta() == 0 && porta_do_soquete(s) == 65535 {
            // Outro soquete antes de soltar este garante outra porta.
            let novo = criar_escuta(e, fila, so_v6);
            // SAFETY: o soquete não foi entregue a ninguém.
            unsafe { closesocket(s) };
            return novo;
        }
        // SAFETY: soquete aberto.
        if unsafe { listen(s, if fila > 0 { fila as i32 } else { SOMAXCONN }) } != 0 {
            return Err(fechar_com_erro(s));
        }
        let m = Manipulador::novo(TipoDeManipulador::Escuta, s);
        m.estado().familia = e.familia();
        if !comecar_a_aceitar(&m) {
            descartar(&m);
            return Err(ErroDoSo { codigo: -1, mensagem: "Failed to start accept".to_string() });
        }
        Ok(descritor_de(m))
    }

    pub(super) fn criar_escuta_unix(_e: &EnderecoSo, _fila: i64) -> ResultadoIo<i64> {
        Err(sem_dominio_unix())
    }

    /// `Socket::CreateBindDatagram`.
    pub(super) fn criar_datagrama(e: &EnderecoSo, reusar_endereco: bool, reusar_porta: bool, ttl: i64) -> ResultadoIo<i64> {
        let s = novo_soquete(e.familia(), SOCK_DGRAM, IPPROTO_UDP)?;
        if reusar_endereco && !definir_opcao_int(s, SOL_SOCKET, SO_REUSEADDR, 1) {
            return Err(fechar_com_erro(s));
        }
        if reusar_porta {
            eprintln!("Dart Socket ERROR: `reusePort` not supported for Windows.");
        }
        let (nivel, nome) = if e.e_ipv6() { (IPPROTO_IPV6, IPV6_MULTICAST_HOPS) } else { (IPPROTO_IP, IP_MULTICAST_TTL) };
        if !definir_opcao_int(s, nivel, nome, ttl as i32) {
            return Err(fechar_com_erro(s));
        }
        // SAFETY: `e` é um `sockaddr` válido.
        if unsafe { bind(s, e.bytes.as_ptr(), e.tamanho_da_familia() as i32) } != 0 {
            return Err(fechar_com_erro(s));
        }
        Ok(descritor_de(Manipulador::novo(TipoDeManipulador::Datagrama, s)))
    }

    fn porta_do_soquete(s: Bruto) -> i64 {
        let mut e = EnderecoSo::vazio();
        let mut n = 128i32;
        // SAFETY: `e` tem 128 bytes.
        if unsafe { getsockname(s, e.bytes.as_mut_ptr(), &mut n) } != 0 {
            return 0;
        }
        e.porta()
    }

    /// `SocketBase::GetPort` (0 em erro).
    pub(super) fn porta_local(fd: i64) -> i64 {
        soquete_bruto(fd).map_or(0, porta_do_soquete)
    }

    /// `SocketBase::GetRemotePeer`: o endereço que o `AcceptEx` deu, ou o
    /// `getpeername`.
    pub(super) fn par_remoto(fd: i64) -> ResultadoIo<(EnderecoSo, i64)> {
        let m = soquete(fd)?;
        if m.tipo == TipoDeManipulador::Cliente {
            if let Some(e) = m.estado().remoto.clone() {
                let porta = e.porta();
                return Ok((e, porta));
            }
        }
        let mut e = EnderecoSo::vazio();
        let mut n = 128i32;
        // SAFETY: `e` tem 128 bytes.
        if unsafe { getpeername(m.bruto, e.bytes.as_mut_ptr(), &mut n) } != 0 {
            return Err(ultimo_erro());
        }
        e.tamanho = n as u32;
        let porta = e.porta();
        Ok((e, porta))
    }

    /// `SocketBase::GetError`: o último erro do manipulador.
    pub(super) fn erro_pendente(fd: i64) -> i32 {
        soquete(fd).map_or(6, |m| m.estado().ultimo_erro as i32)
    }

    /// `SocketBase::GetType` (`GetFileType`).
    pub(super) fn tipo_do_descritor(fd: i64) -> ResultadoIo<i64> {
        let m = soquete(fd)?;
        // SAFETY: consulta o tipo de um handle aberto.
        Ok(match unsafe { GetFileType(m.bruto) } {
            FILE_TYPE_CHAR => 0,
            FILE_TYPE_PIPE => 1,
            FILE_TYPE_DISK => 2,
            _ => match unsafe { GetLastError() } {
                0 => 4,
                c => return Err(ErroDoSo::do_codigo(c as i32)),
            },
        })
    }

    pub(super) fn codigo_em_andamento() -> i64 {
        WSAEINPROGRESS
    }

    pub(super) fn e_erro_de_bind(erro: i64) -> bool {
        [WSAEADDRINUSE, WSAEADDRNOTAVAIL, WSAEINVAL].contains(&erro)
    }

    /// `Socket_GetOption`: 0 `TCP_NODELAY`, 1 laço de multicast, 2 TTL de
    /// multicast, 4 `SO_BROADCAST`.
    pub(super) fn ler_opcao(fd: i64, opcao: i64, v4: bool) -> Option<i64> {
        let s = soquete_bruto(fd)?;
        match opcao {
            0 => ler_opcao_int(s, IPPROTO_TCP, TCP_NODELAY),
            1 => {
                let (nivel, nome) = if v4 { (IPPROTO_IP, IP_MULTICAST_LOOP) } else { (IPPROTO_IPV6, IPV6_MULTICAST_LOOP) };
                ler_opcao_int(s, nivel, nome)
            }
            2 => {
                let (nivel, nome) = if v4 { (IPPROTO_IP, IP_MULTICAST_TTL) } else { (IPPROTO_IPV6, IPV6_MULTICAST_HOPS) };
                ler_opcao_int(s, nivel, nome)
            }
            4 => ler_opcao_int(s, SOL_SOCKET, SO_BROADCAST),
            _ => None,
        }
    }

    pub(super) fn definir_opcao(fd: i64, opcao: i64, v4: bool, valor: i64) -> bool {
        let (nivel, nome) = match opcao {
            0 => (IPPROTO_TCP, TCP_NODELAY),
            1 if v4 => (IPPROTO_IP, IP_MULTICAST_LOOP),
            1 => (IPPROTO_IPV6, IPV6_MULTICAST_LOOP),
            2 if v4 => (IPPROTO_IP, IP_MULTICAST_TTL),
            2 => (IPPROTO_IPV6, IPV6_MULTICAST_HOPS),
            4 => (SOL_SOCKET, SO_BROADCAST),
            _ => return false,
        };
        soquete_bruto(fd).is_some_and(|s| definir_opcao_int(s, nivel, nome, valor as i32))
    }

    pub(super) fn definir_opcao_bruta(fd: i64, nivel: i64, opcao: i64, dados: &[u8]) -> bool {
        let Some(s) = soquete_bruto(fd) else { return false };
        // SAFETY: `dados` vive durante a chamada.
        unsafe { setsockopt(s, nivel as i32, opcao as i32, dados.as_ptr(), dados.len() as i32) == 0 }
    }

    pub(super) fn ler_opcao_bruta(fd: i64, nivel: i64, opcao: i64, dados: &mut Vec<u8>) -> bool {
        let Some(s) = soquete_bruto(fd) else { return false };
        let mut n = dados.len() as i32;
        // SAFETY: `dados` é gravável com `n` bytes.
        let ok = unsafe { getsockopt(s, nivel as i32, opcao as i32, dados.as_mut_ptr(), &mut n) } == 0;
        dados.truncate(n.max(0) as usize);
        ok
    }

    pub(super) fn constante_de_opcao_bruta(chave: i64) -> Option<i64> {
        Some(i64::from(match chave {
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
    // Nomes e interfaces.

    /// `SocketBase::LookupAddress` (primeiro com `AI_ADDRCONFIG`).
    pub(super) fn resolver_nome(host: &str, tipo: i64) -> ResultadoIo<Vec<EnderecoSo>> {
        iniciar_winsock();
        let nome = largo(host);
        let familia = match tipo {
            TIPO_IPV4 => i32::from(AF_INET),
            TIPO_IPV6 => i32::from(AF_INET6),
            _ => 0,
        };
        let mut dicas = InfoDeEndereco {
            ai_flags: AI_ADDRCONFIG,
            ai_family: familia,
            ai_socktype: SOCK_STREAM,
            ai_protocol: IPPROTO_TCP,
            ai_addrlen: 0,
            ai_canonname: std::ptr::null_mut(),
            ai_addr: std::ptr::null(),
            ai_next: std::ptr::null_mut(),
        };
        let mut res: *mut InfoDeEndereco = std::ptr::null_mut();
        // SAFETY: `nome`, `dicas` e `res` vivem durante as chamadas.
        let mut status = unsafe { GetAddrInfoW(nome.as_ptr(), std::ptr::null(), &dicas, &mut res) };
        if status != 0 {
            dicas.ai_flags = 0;
            status = unsafe { GetAddrInfoW(nome.as_ptr(), std::ptr::null(), &dicas, &mut res) };
            if status != 0 {
                return Err(ErroDoSo::do_codigo(status));
            }
        }
        let mut saida = Vec::new();
        let mut atual = res;
        while !atual.is_null() {
            // SAFETY: nó da lista devolvida por `GetAddrInfoW`.
            let i = unsafe { &*atual };
            if i.ai_family == i32::from(AF_INET) || i.ai_family == i32::from(AF_INET6) {
                saida.push(EnderecoSo::de_ponteiro(i.ai_addr, i.ai_addrlen as u32));
            }
            atual = i.ai_next;
        }
        // SAFETY: a lista veio de `GetAddrInfoW`.
        unsafe { FreeAddrInfoW(res) };
        Ok(saida)
    }

    /// `SocketBase::ReverseLookup`.
    pub(super) fn nome_do_endereco(e: &EnderecoSo) -> ResultadoIo<String> {
        iniciar_winsock();
        let mut b = [0u16; 1025];
        // SAFETY: o buffer tem `NI_MAXHOST` unidades.
        let status = unsafe {
            GetNameInfoW(e.bytes.as_ptr(), e.tamanho_da_familia() as i32, b.as_mut_ptr(), b.len() as u32, std::ptr::null_mut(), 0, NI_NAMEREQD)
        };
        if status != 0 {
            return Err(ErroDoSo::do_codigo(status));
        }
        Ok(de_largo(&b))
    }

    /// `SocketBase::ListInterfaces` (`GetAdaptersAddresses`, sem anycast,
    /// multicast e DNS): (endereço, nome amigável, índice).
    pub(super) fn listar_interfaces(tipo: i64) -> ResultadoIo<Vec<(EnderecoSo, String, i64)>> {
        iniciar_winsock();
        const GAA_FLAG_SKIP_ANYCAST: u32 = 0x2;
        const GAA_FLAG_SKIP_MULTICAST: u32 = 0x4;
        const GAA_FLAG_SKIP_DNS_SERVER: u32 = 0x8;
        let familia = match tipo {
            TIPO_IPV4 => u32::from(AF_INET),
            TIPO_IPV6 => u32::from(AF_INET6),
            _ => 0,
        };
        let bandeiras = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
        // O tamanho muda entre as chamadas se uma interface aparece: repete
        // enquanto faltar espaço.
        let mut tamanho = 16 * 1024u32;
        let mut buf: Vec<u64>;
        loop {
            buf = vec![0; (tamanho as usize).div_ceil(8)];
            // SAFETY: o buffer tem `tamanho` bytes, alinhado a 8.
            let r = unsafe { GetAdaptersAddresses(familia, bandeiras, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut tamanho) };
            match r {
                0 => break,
                ERROR_BUFFER_OVERFLOW => continue,
                c => return Err(ErroDoSo::do_codigo(c as i32)),
            }
        }
        let mut saida = Vec::new();
        let mut a = buf.as_ptr().cast::<Adaptador>();
        while !a.is_null() {
            // SAFETY: nó da lista que o sistema gravou no buffer.
            let ad = unsafe { &*a };
            // SAFETY: o nome amigável é um texto terminado em NUL.
            let nome = unsafe { de_ponteiro_largo(ad.nome_amigavel) };
            let indice = if ad.indice_ipv6 != 0 { ad.indice_ipv6 } else { ad.indice };
            let mut u = ad.primeiro_unicast;
            while !u.is_null() {
                // SAFETY: nó da lista de endereços do adaptador.
                let un = unsafe { &*u };
                saida.push((EnderecoSo::de_ponteiro(un.endereco, un.tamanho_do_endereco.max(0) as u32), nome.clone(), i64::from(indice)));
                u = un.proximo;
            }
            a = ad.proximo;
        }
        Ok(saida)
    }

    /// `SocketBase::ParseAddress` (`InetPtonW`).
    pub(super) fn interpretar_endereco(texto: &str) -> Option<Vec<u8>> {
        iniciar_winsock();
        let v6 = texto.contains(':');
        let mut b = [0u8; 16];
        let familia = if v6 { AF_INET6 } else { AF_INET };
        let t = largo(texto);
        // SAFETY: `b` tem 16 bytes.
        (unsafe { InetPtonW(i32::from(familia), t.as_ptr(), b.as_mut_ptr()) } == 1).then(|| b[..if v6 { 16 } else { 4 }].to_vec())
    }
}

#[cfg(windows)]
use so_windows::*;
