// Runtime nativo: os soquetes do `dart:io` da VM (`runtime/bin/socket.cc`):
// TCP (cliente e servidor, com o registro de soquetes de escuta
// compartilhados), UDP, domínio Unix, endereços (`InternetAddress`) e a
// resolução de nomes do IOService (pedidos 32–34).
//
// Os natives devolvem o que os da VM devolvem (`true`, um `OSError`, uma
// `Uint8List`…) e lançam o que eles lançam; o descritor fica no
// [`SoqueteNativo`] do campo nativo (`io_eventos.rs`), não bloqueante, e o
// manipulador de eventos avisa quando há o que ler ou escrever.
//
// Os natives não dependem do sistema: a camada do sistema (`EnderecoSo`,
// `criar_conexao`, `ler_de`, `resolver_nome`…) tem os mesmos nomes em
// `io_soquetes_unix.rs` (Linux e macOS) e em `io_windows_soquetes.rs`
// (Winsock, sobre a porta de conclusão de `io_windows_eventos.rs`).

/// `SocketAddress::TYPE_*`.
const TIPO_IPV4: i64 = 0;
const TIPO_IPV6: i64 = 1;
const TIPO_UNIX: i64 = 2;

/// Um `sockaddr_storage` e o tamanho usado.
#[derive(Clone)]
struct EnderecoSo {
    bytes: [u8; 128],
    tamanho: u32,
}

/// O endereço de um `Uint8List` Dart (4 ou 16 bytes).
fn endereco_de_lista(h: i64) -> Option<EnderecoSo> {
    EnderecoSo::de_ip(&bytes_da_lista_tipada(h)?)
}

/// A `ArgumentError` de um endereço mal formado (a VM propaga um erro da
/// API: "Unexpected type for socket address").
fn lancar_endereco_invalido() {
    let msg = alocar_str("Unexpected type for socket address");
    let e = com_raizes(&[msg], || dartforge_argument_error_new(msg, 0));
    com_raizes(&[e], || dartforge_exception_throw(e, 3));
}

/// Lança o `OSError` (os natives de soquete lançam em vez de devolver).
fn lancar_os_error(e: &ErroDoSo) {
    let erro = e.para_dart();
    if dartforge_exception_pending() == 0 {
        com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
    }
}

/// Lança o `ArgumentError(mensagem)` (os erros da API da VM).
fn lancar_erro_de_argumento(mensagem: &str) {
    let msg = alocar_str(mensagem);
    let e = com_raizes(&[msg], || dartforge_argument_error_new(msg, 0));
    com_raizes(&[e], || dartforge_exception_throw(e, 3));
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
    let mut e = endereco_de_lista(addr)?;
    e.gravar_porta(porta);
    e.gravar_escopo(escopo);
    Some(e)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateConnect(this: i64, addr: i64, porta: i64, escopo: i64) -> i64 {
    let Some(e) = endereco_de_destino(addr, porta, escopo) else {
        lancar_endereco_invalido();
        return 0;
    };
    soquete_criado(this, criar_conexao(&e), FinalizadorDeSoquete::Normal)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateBindConnect(this: i64, addr: i64, porta: i64, origem: i64, porta_origem: i64, escopo: i64) -> i64 {
    let (Some(e), Some(o)) = (endereco_de_destino(addr, porta, escopo), endereco_de_destino(origem, porta_origem, 0)) else {
        lancar_endereco_invalido();
        return 0;
    };
    soquete_criado(this, criar_conexao_com_origem(&e, &o), FinalizadorDeSoquete::Normal)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateUnixDomainConnect(this: i64, caminho: i64, _ns: i64) -> i64 {
    let r = EnderecoSo::de_caminho_unix(&utf8_de_texto(caminho)).and_then(|e| criar_conexao_unix(&e, None));
    soquete_criado(this, r, FinalizadorDeSoquete::Normal)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateUnixDomainBindConnect(this: i64, caminho: i64, origem: i64, _ns: i64) -> i64 {
    let r = EnderecoSo::de_caminho_unix(&utf8_de_texto(caminho)).and_then(|e| {
        let o = EnderecoSo::de_caminho_unix(&utf8_de_texto(origem))?;
        criar_conexao_unix(&e, Some(&o))
    });
    soquete_criado(this, r, FinalizadorDeSoquete::Normal)
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
    let Some(e) = endereco_de_destino(addr, porta, escopo) else {
        lancar_endereco_invalido();
        return 0;
    };
    let mut reg = registro_de_escuta();
    let porta = e.porta();
    let (so_v6, compartilhado) = (so_v6 != 0, compartilhado != 0);
    if porta > 0 {
        if let Some(existente) = reg.soquetes.iter_mut().find(|s| s.porta == porta && s.endereco == e.ip()) {
            let mensagem = if !existente.compartilhado || !compartilhado {
                Some("The shared flag to bind() needs to be `true` if binding multiple times on the same (address, port) combination.")
            } else if existente.so_v6 != so_v6 {
                Some("The v6Only flag to bind() needs to be the same if binding multiple times on the same (address, port) combination.")
            } else {
                None
            };
            if let Some(m) = mensagem {
                drop(reg);
                return ErroDoSo { codigo: -1, mensagem: m.to_string() }.para_dart();
            }
            existente.usos += 1;
            let fd = existente.descritor;
            let p = SoqueteNativo::novo(reter_descritor(fd));
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
    reg.soquetes.push(SoqueteDeEscuta {
        endereco: e.ip(),
        porta: porta_alocada,
        so_v6,
        compartilhado,
        usos: 1,
        descritor: fd,
        caminho_unix: None,
    });
    reg.por_soquete.insert(p, fd);
    drop(reg);
    reusar_soquete_no_objeto(this, p, FinalizadorDeSoquete::Escuta);
    dart_bool(true)
}

/// `ServerSocket_CreateUnixDomainBindListen`: como o de TCP, com o
/// compartilhamento pelo caminho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ServerSocket_CreateUnixDomainBindListen(this: i64, caminho: i64, fila: i64, compartilhado: u8, _ns: i64) -> i64 {
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
        let p = SoqueteNativo::novo(reter_descritor(fd));
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

/// `ServerSocket_Accept(socket)`: `true` com a conexão nova no objeto.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ServerSocket_Accept(this: i64, novo: i64) -> u8 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    let fd = aceitar(s.descritor());
    if fd >= 0 {
        definir_soquete_no_objeto(novo, fd, FinalizadorDeSoquete::Normal);
        return 1;
    }
    0
}

/// `Socket_Available`: os bytes prontos (1 se o sistema não sabe dizer).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_Available(this: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    let n = disponivel(s);
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
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    if n < 0 {
        lancar_os_error(&ErroDoSo::argumento_invalido());
        return 0;
    }
    let mut buf = vec![0u8; n as usize];
    match ler_de(s, &mut buf) {
        Ok(lidos) if lidos as i64 == n => dart_bytes(buf),
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

/// `Socket_WriteList(buffer, offset, bytes)`: os bytes escritos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_WriteList(this: i64, buffer: i64, inicio: i64, n: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    let bytes = bytes_da_lista_tipada(buffer).unwrap_or_default();
    let inicio = (inicio.max(0) as usize).min(bytes.len());
    let fim = (inicio + n.max(0) as usize).min(bytes.len());
    match escrever_em(s, &bytes[inicio..fim]) {
        Ok(escritos) => escritos as i64,
        Err(e) => {
            lancar_os_error(&e);
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_HasPendingWrite(this: i64) -> u8 {
    soquete_do_objeto(this).map_or(0, |s| u8::from(escrita_pendente(s)))
}

/// `Socket_GetPort`: a porta local, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetPort(this: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    let p = porta_local(s.descritor());
    if p > 0 {
        return dart_int(p);
    }
    ultimo_erro().para_dart()
}

/// `Socket_GetRemotePeer`: `[[tipo, texto, bytes], porta]` (`[tipo,
/// texto]` no domínio Unix).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetRemotePeer(this: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
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
}

/// `Socket_GetError`: o erro pendente como `OSError`, ou `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetError(this: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    let c = erro_pendente(s.descritor());
    if c != 0 { ErroDoSo::do_codigo(c).para_dart() } else { 0 }
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
    dart_ou_erro(tipo_do_descritor(s.descritor()), dart_int)
}

/// `Socket_GetStdioHandle(socket, num)`: a entrada/saída padrão `num` como
/// soquete (a entrada assíncrona de `stdin`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetStdioHandle(soquete: i64, num: i64) -> u8 {
    let fd = manipulador_padrao(num.clamp(0, 2));
    if fd >= 0 {
        definir_soquete_no_objeto(soquete, fd, FinalizadorDeSoquete::Stdio);
    }
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
/// `id` (o `_NativeSocket` criado para um descritor já aberto: os pipes dos
/// sinais).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SetSocketId(this: i64, id: i64, bandeiras: i64) {
    let f = if bandeiras & (1 << 21) != 0 { FinalizadorDeSoquete::Sinal } else { FinalizadorDeSoquete::Normal };
    definir_soquete_no_objeto(this, id, f);
}

/// `SocketBase_IsBindError(errno)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SocketBase_IsBindError(_this: i64, erro: i64) -> u8 {
    u8::from(e_erro_de_bind(erro))
}

/// `OSError_inProgressErrorCode`: o "operação em andamento" do sistema.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_OSError_inProgressErrorCode() -> i64 {
    codigo_em_andamento()
}

/// `Socket_GetOption(option, protocol)`: `TCP_NODELAY`, laço e TTL de
/// multicast, `SO_BROADCAST`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetOption(this: i64, opcao: i64, protocolo: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    match ler_opcao(s.descritor(), opcao, protocolo == TIPO_IPV4) {
        Some(v) if opcao == 2 => dart_int(v),
        Some(v) => dart_bool(v != 0),
        None => {
            lancar_os_error(&ultimo_erro());
            0
        }
    }
}

/// `Socket_SetOption(option, protocol, value)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SetOption(this: i64, opcao: i64, protocolo: i64, valor: i64) {
    let Some(s) = soquete_do_objeto(this) else { return };
    if !(0..=4).contains(&opcao) || opcao == 3 {
        lancar_erro_de_argumento("option to setOption() is outside expected range");
        return;
    }
    let v = HEAP.with(|h| {
        let h = h.borrow();
        match h.bool_de(valor) {
            Some(b) => i64::from(b),
            None => h.int_de(valor).unwrap_or(0),
        }
    });
    if !definir_opcao(s.descritor(), opcao, protocolo == TIPO_IPV4, v) {
        lancar_os_error(&ultimo_erro());
    }
}

/// `Socket_SetRawOption(level, option, data)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SetRawOption(this: i64, nivel: i64, opcao: i64, dados: i64) {
    let Some(s) = soquete_do_objeto(this) else { return };
    let b = bytes_da_lista_tipada(dados).unwrap_or_default();
    if !definir_opcao_bruta(s.descritor(), nivel, opcao, &b) {
        lancar_os_error(&ultimo_erro());
    }
}

/// `Socket_JoinMulticast(addr, interfaceAddr, interfaceIndex)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_JoinMulticast(this: i64, addr: i64, iface: i64, indice: i64) {
    multicast(this, addr, iface, indice, true);
}

/// `Socket_LeaveMulticast(addr, interfaceAddr, interfaceIndex)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_LeaveMulticast(this: i64, addr: i64, iface: i64, indice: i64) {
    multicast(this, addr, iface, indice, false);
}

/// Entra (`entrar`) ou sai de um grupo multicast, como a VM em cada sistema
/// (`socket_base_{linux,macos,win}.cc`): no Linux, `MCAST_JOIN_GROUP` com o
/// `group_req` (o índice da interface e o endereço do grupo); no macOS e no
/// Windows, `ip_mreq` (IPv4, pelo endereço da interface) ou `ipv6_mreq`
/// (IPv6, pelo índice). O endereço do grupo são os 4 ou 16 bytes do
/// `_InternetAddress._in_addr`.
fn multicast(this: i64, addr: i64, iface: i64, indice: i64, entrar: bool) {
    let Some(s) = soquete_do_objeto(this) else { return };
    let Some(grupo) = bytes_da_lista_tipada(addr).filter(|b| b.len() == 4 || b.len() == 16) else {
        lancar_erro_de_argumento("Invalid multicast address");
        return;
    };
    let iface = if iface == 0 { None } else { bytes_da_lista_tipada(iface) };
    let v4 = grupo.len() == 4;
    let (nivel, opcao, dados) = opcao_de_multicast(&grupo, iface.as_deref(), indice, v4, entrar);
    if !definir_opcao_bruta(s.descritor(), nivel, opcao, &dados) {
        lancar_os_error(&ultimo_erro());
    }
}

/// `IPPROTO_IP` e `IPPROTO_IPV6` (os mesmos nos três sistemas).
const NIVEL_IP: i64 = 0;
const NIVEL_IPV6: i64 = 41;

#[cfg(any(target_os = "linux", target_os = "android"))]
fn opcao_de_multicast(grupo: &[u8], _iface: Option<&[u8]>, indice: i64, v4: bool, entrar: bool) -> (i64, i64, Vec<u8>) {
    // `struct group_req { uint32_t gr_interface; struct sockaddr_storage
    // gr_group; }`: o `sockaddr_storage` alinhado em 8 (136 bytes).
    const MCAST_JOIN_GROUP: i64 = 42;
    const MCAST_LEAVE_GROUP: i64 = 45;
    let mut r = vec![0u8; 8 + 128];
    r[0..4].copy_from_slice(&(indice as u32).to_ne_bytes());
    if v4 {
        r[8..10].copy_from_slice(&2u16.to_ne_bytes()); // AF_INET
        r[12..16].copy_from_slice(grupo);
    } else {
        r[8..10].copy_from_slice(&10u16.to_ne_bytes()); // AF_INET6
        r[16..32].copy_from_slice(grupo);
    }
    let opcao = if entrar { MCAST_JOIN_GROUP } else { MCAST_LEAVE_GROUP };
    (if v4 { NIVEL_IP } else { NIVEL_IPV6 }, opcao, r)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn opcao_de_multicast(grupo: &[u8], iface: Option<&[u8]>, indice: i64, v4: bool, entrar: bool) -> (i64, i64, Vec<u8>) {
    // `IP_ADD_MEMBERSHIP`/`IP_DROP_MEMBERSHIP` e `IPV6_JOIN_GROUP`/
    // `IPV6_LEAVE_GROUP` valem 12 e 13 no macOS e no Windows.
    let opcao = if entrar { 12 } else { 13 };
    if v4 {
        // `struct ip_mreq { in_addr imr_multiaddr; in_addr imr_interface; }`
        let mut r = vec![0u8; 8];
        r[0..4].copy_from_slice(grupo);
        if let Some(i) = iface.filter(|i| i.len() == 4) {
            r[4..8].copy_from_slice(i);
        }
        (NIVEL_IP, opcao, r)
    } else {
        // `struct ipv6_mreq { in6_addr ipv6mr_multiaddr; unsigned int ipv6mr_interface; }`
        let mut r = vec![0u8; 20];
        r[0..16].copy_from_slice(grupo);
        r[16..20].copy_from_slice(&(indice as u32).to_ne_bytes());
        (NIVEL_IPV6, opcao, r)
    }
}

/// `Socket_GetRawOption(level, option, data)`: grava o valor em `data`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_GetRawOption(this: i64, nivel: i64, opcao: i64, dados: i64) {
    let Some(s) = soquete_do_objeto(this) else { return };
    let mut b = bytes_da_lista_tipada(dados).unwrap_or_default();
    if !ler_opcao_bruta(s.descritor(), nivel, opcao, &mut b) {
        lancar_os_error(&ultimo_erro());
        return;
    }
    gravar_bytes_na_lista(dados, 0, &b);
}

/// `RawSocketOption_GetOptionValue(key)`: as constantes do sistema.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_RawSocketOption_GetOptionValue(chave: i64) -> i64 {
    match constante_de_opcao_bruta(chave) {
        Some(v) => v,
        None => {
            lancar_erro_de_argumento("option to getOptionValue() is outside expected range");
            0
        }
    }
}

/// `Socket_Fatal(msg)`: erro irrecuperável do `dart:io` (a VM aborta).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_Fatal(msg: i64) {
    let texto = if msg == 0 { String::from("(null)") } else { HEAP.with(|h| h.borrow().texto(msg).map(|t| t.para_string()).unwrap_or_default()) };
    eprintln!("Fatal error in dart:io (socket): {texto}");
    std::process::abort();
}

// ---------------------------------------------------------------------------
// UDP.

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_CreateBindDatagram(this: i64, addr: i64, porta: i64, reusar_endereco: u8, reusar_porta: u8, ttl: i64) -> i64 {
    let Some(e) = endereco_de_destino(addr, porta, 0) else {
        lancar_endereco_invalido();
        return 0;
    };
    soquete_criado(this, criar_datagrama(&e, reusar_endereco != 0, reusar_porta != 0, ttl), FinalizadorDeSoquete::Normal)
}

/// `Socket_AvailableDatagram`: se há um datagrama.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_AvailableDatagram(this: i64) -> u8 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    u8::from(ha_datagrama(s.descritor()))
}

/// O tamanho máximo de um datagrama UDP (o buffer de recepção da VM).
const TAMANHO_MAXIMO_DE_DATAGRAMA: usize = 65536;

/// `Socket_RecvFrom`: o `Datagram` recebido (pelo `_makeDatagram` do
/// patch), ou `null` se não havia nenhum.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_RecvFrom(this: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    let (buf, e) = match receber_datagrama(s.descritor(), TAMANHO_MAXIMO_DE_DATAGRAMA) {
        Ok(Some(x)) => x,
        Ok(None) => return 0,
        Err(erro) => {
            lancar_os_error(&erro);
            return 0;
        }
    };
    let Some(f) = ajudante("_dartforgeDatagrama") else {
        panic!("bug do compilador: dart:io sem `_dartforgeDatagrama` registrado");
    };
    // SAFETY: registrada pela sobreposição de `dart:io` com a assinatura
    // (`Uint8List`, `String`, `Uint8List`, `int`, `int`).
    let g = { let alvo_dart: usize = f; move |a0: i64, a1: i64, a2: i64, a3: i64, a4: i64| -> i64 { dart_r5(alvo_dart, a0, a1, a2, a3, a4) } };
    let dados = dart_bytes(buf);
    let texto = com_raizes(&[dados], || alocar_str(&e.texto()));
    let ip = com_raizes(&[dados, texto], || dart_bytes(e.ip()));
    com_raizes(&[dados, texto, ip], || g(dados, texto, ip, e.porta(), e.tipo()))
}

/// `Socket_SendTo(buffer, offset, bytes, address, port)`: os bytes
/// enviados (0 se o soquete está cheio).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SendTo(this: i64, buffer: i64, inicio: i64, n: i64, addr: i64, porta: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    let Some(e) = endereco_de_destino(addr, porta, 0) else {
        lancar_endereco_invalido();
        return 0;
    };
    let bytes = bytes_da_lista_tipada(buffer).unwrap_or_default();
    let inicio = (inicio.max(0) as usize).min(bytes.len());
    let fim = (inicio + n.max(0) as usize).min(bytes.len());
    match enviar_datagrama(s.descritor(), &bytes[inicio..fim], &e) {
        Ok(n) => n as i64,
        Err(erro) => {
            lancar_os_error(&erro);
            0
        }
    }
}

// ---------------------------------------------------------------------------
// `InternetAddress`.

/// `InternetAddress_Parse(address)`: os bytes do endereço numérico, ou
/// `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_InternetAddress_Parse(texto: i64) -> i64 {
    let t = HEAP.with(|h| h.borrow().texto(texto).map(|t| t.para_string()).unwrap_or_default());
    interpretar_endereco(&t).map_or(0, dart_bytes)
}

/// `InternetAddress_RawAddrToString(address)`: o texto numérico.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_InternetAddress_RawAddrToString(addr: i64) -> i64 {
    let Some(e) = endereco_de_lista(addr) else {
        lancar_endereco_invalido();
        return 0;
    };
    alocar_str(&e.texto())
}

/// `InternetAddress_ParseScopedLinkLocalAddress(address)`: o escopo de um
/// IPv6 de enlace local com `%interface`, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_InternetAddress_ParseScopedLinkLocalAddress(texto: i64) -> i64 {
    let t = HEAP.with(|h| h.borrow().texto(texto).map(|t| t.para_string()).unwrap_or_default());
    match resolver_nome(&t, TIPO_IPV6) {
        Ok(v) if !v.is_empty() => dart_int(v[0].escopo()),
        Ok(_) => ErroDoSo::do_codigo(codigo_do_so::INVALIDO).para_dart(),
        Err(e) => e.para_dart(),
    }
}

// ---------------------------------------------------------------------------
// Os pedidos de rede do IOService (32 lookup, 33 interfaces, 34 reverso).

/// `Socket::LookupRequest(host, type)`: `[0, [tipo, texto, bytes, escopo]…]`.
fn pedido_de_resolucao(d: &[Portavel]) -> Portavel {
    let (Some(host), Some(tipo)) = (d.first().and_then(Portavel::str), d.get(1).and_then(Portavel::int)) else {
        return argumento_invalido();
    };
    resposta(resolver_nome(host, tipo), |v| {
        let mut l = vec![Portavel::Int(0)];
        for e in v {
            l.push(Portavel::Lista(vec![Portavel::Int(e.tipo()), Portavel::Str(e.texto()), Portavel::Bytes(e.ip()), Portavel::Int(e.escopo())]));
        }
        Portavel::Lista(l)
    })
}

/// `Socket::ListInterfacesRequest(type)`: `[0, [tipo, texto, bytes, nome,
/// índice]…]`.
fn pedido_de_interfaces(d: &[Portavel]) -> Portavel {
    let Some(tipo) = d.first().and_then(Portavel::int) else {
        return argumento_invalido();
    };
    resposta(listar_interfaces(tipo), |v| {
        let mut l = vec![Portavel::Int(0)];
        for (e, nome, indice) in v {
            l.push(Portavel::Lista(vec![Portavel::Int(e.tipo()), Portavel::Str(e.texto()), Portavel::Bytes(e.ip()), Portavel::Str(nome), Portavel::Int(indice)]));
        }
        Portavel::Lista(l)
    })
}

/// `Socket::ReverseLookupRequest(address)`: o nome do endereço.
fn pedido_de_resolucao_reversa(d: &[Portavel]) -> Portavel {
    let Some(e) = d.first().and_then(Portavel::bytes).and_then(EnderecoSo::de_ip) else {
        return argumento_invalido();
    };
    resposta(nome_do_endereco(&e), Portavel::Str)
}

// ---------------------------------------------------------------------------
// `RawSynchronousSocket` (`sync_socket.cc` da VM): um TCP bloqueante, sem o
// manipulador de eventos. O `std::net::TcpStream` do Rust é o mesmo soquete
// bloqueante nos três sistemas; ele mora numa caixa no campo nativo do
// `_NativeSynchronousSocket` e é fechado no `closeSync` ou na coleta.

/// O soquete síncrono do objeto (`None` depois do `closeSync`).
type SoqueteSincrono = Option<std::net::TcpStream>;

fn soquete_sincrono<'a>(objeto: i64) -> Option<&'a mut SoqueteSincrono> {
    let p = campo_nativo(objeto);
    if p == 0 {
        lancar_erro_interno("No native peer");
        return None;
    }
    // SAFETY: a caixa criada em `CreateConnectSync`, viva até a coleta do
    // objeto (o finalizador a solta); o isolado é uma thread só.
    Some(unsafe { &mut *(p as *mut SoqueteSincrono) })
}

fn finalizar_soquete_sincrono(p: usize) {
    // SAFETY: a caixa de `CreateConnectSync`, solta só aqui.
    drop(unsafe { Box::from_raw(p as *mut SoqueteSincrono) });
}

/// O `OSError` de um erro do sistema do `std`.
fn erro_de_io(e: &std::io::Error) -> i64 {
    ErroDoSo::do_codigo(e.raw_os_error().unwrap_or(0)).para_dart()
}

/// `[tipo, texto, bytes]` de um IP.
fn dart_endereco_ip(ip: std::net::IpAddr) -> i64 {
    let (tipo, bytes) = match ip {
        std::net::IpAddr::V4(a) => (TIPO_IPV4, a.octets().to_vec()),
        std::net::IpAddr::V6(a) => (TIPO_IPV6, a.octets().to_vec()),
    };
    let texto = alocar_str(&ip.to_string());
    com_raizes(&[texto], || {
        let b = dart_bytes(bytes);
        com_raizes(&[b], || dart_lista_fixa(&[dart_int(tipo), texto, b]))
    })
}

/// `SynchronousSocket_LookupRequest(host, type)`: `[[tipo, texto, bytes]…]`
/// ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_LookupRequest(host: i64, tipo: i64) -> i64 {
    let host = String::from_utf8_lossy(&utf8_de_texto(host)).into_owned();
    match resolver_nome(&host, tipo) {
        Ok(v) => {
            let mut itens = Vec::with_capacity(v.len());
            let quadro = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(v.len().max(1)));
            for (i, e) in v.iter().enumerate() {
                let texto = alocar_str(&e.texto());
                let item = com_raizes(&[texto], || {
                    let b = dart_bytes(e.ip());
                    com_raizes(&[b], || dart_lista_fixa(&[dart_int(e.tipo()), texto, b]))
                });
                HEAP.with(|h| h.borrow_mut().set_root(quadro, i, item));
                itens.push(item);
            }
            let r = dart_lista_fixa(&itens);
            HEAP.with(|h| h.borrow_mut().pop_frame(quadro));
            r
        }
        Err(e) => e.para_dart(),
    }
}

/// `SynchronousSocket_CreateConnectSync(addr, port)`: `null` conectado, ou
/// o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_CreateConnectSync(this: i64, addr: i64, porta: i64) -> i64 {
    let ip = match bytes_da_lista_tipada(addr).as_deref() {
        Some(&[a, b, c, d]) => std::net::IpAddr::from([a, b, c, d]),
        Some(b) if b.len() == 16 => {
            let mut o = [0u8; 16];
            o.copy_from_slice(b);
            std::net::IpAddr::from(o)
        }
        _ => {
            lancar_endereco_invalido();
            return 0;
        }
    };
    match std::net::TcpStream::connect((ip, porta as u16)) {
        Ok(s) => {
            let p = Box::into_raw(Box::new(Some(s))) as i64;
            gravar_campo_nativo(this, p);
            anexar_finalizador(this, finalizar_soquete_sincrono, p as usize);
            0
        }
        Err(e) => erro_de_io(&e),
    }
}

/// `SynchronousSocket_Available`: os bytes já recebidos e não lidos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_Available(this: i64) -> i64 {
    let Some(Some(s)) = soquete_sincrono(this) else { return dart_int(0) };
    dart_int(bytes_prontos(s).max(0))
}

#[cfg(unix)]
fn bytes_prontos(s: &std::net::TcpStream) -> i64 {
    use std::os::fd::AsRawFd;
    bytes_disponiveis(i64::from(s.as_raw_fd()))
}

#[cfg(windows)]
fn bytes_prontos(s: &std::net::TcpStream) -> i64 {
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
    if let Some(s) = soquete_sincrono(this) {
        s.take();
    }
}

/// `SynchronousSocket_GetPort`: a porta local ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_GetPort(this: i64) -> i64 {
    let Some(Some(s)) = soquete_sincrono(this) else { return 0 };
    match s.local_addr() {
        Ok(a) => dart_int(i64::from(a.port())),
        Err(e) => erro_de_io(&e),
    }
}

/// `SynchronousSocket_GetRemotePeer`: `[[tipo, texto, bytes], porta]` ou o
/// `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_GetRemotePeer(this: i64) -> i64 {
    let Some(Some(s)) = soquete_sincrono(this) else { return 0 };
    match s.peer_addr() {
        Ok(a) => {
            let e = dart_endereco_ip(a.ip());
            com_raizes(&[e], || dart_lista_fixa(&[e, dart_int(i64::from(a.port()))]))
        }
        Err(e) => erro_de_io(&e),
    }
}

/// `SynchronousSocket_Read(len)`: até `len` bytes (bloqueia até chegar
/// algum); `null` no fim do fluxo; ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_Read(this: i64, len: i64) -> i64 {
    use std::io::Read;
    let Some(Some(s)) = soquete_sincrono(this) else { return 0 };
    let len = len.max(0) as usize;
    let mut buf = vec![0u8; len];
    match s.read(&mut buf) {
        Ok(0) => 0,
        Ok(n) => {
            buf.truncate(n);
            dart_bytes(buf)
        }
        Err(e) => erro_de_io(&e),
    }
}

/// `SynchronousSocket_ReadList(buffer, offset, bytes)`: grava em `buffer`
/// e devolve quantos leu (0 no fim do fluxo), ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_ReadList(this: i64, buffer: i64, inicio: i64, n: i64) -> i64 {
    use std::io::Read;
    let Some(Some(s)) = soquete_sincrono(this) else { return 0 };
    let inicio = inicio.max(0) as usize;
    let n = n.max(0) as usize;
    let mut buf = vec![0u8; n];
    match s.read(&mut buf) {
        Ok(lidos) => {
            gravar_bytes_na_lista(buffer, inicio, &buf[..lidos]);
            dart_int(lidos as i64)
        }
        Err(e) => erro_de_io(&e),
    }
}

/// `SynchronousSocket_WriteList(buffer, offset, bytes)`: escreve tudo e
/// devolve quantos, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_WriteList(this: i64, buffer: i64, inicio: i64, n: i64) -> i64 {
    use std::io::Write;
    let Some(Some(s)) = soquete_sincrono(this) else { return 0 };
    let inicio = inicio.max(0) as usize;
    let n = n.max(0) as usize;
    let bytes = bytes_da_lista_tipada(buffer).unwrap_or_default();
    let inicio = inicio.min(bytes.len());
    let fim = (inicio + n).min(bytes.len());
    match s.write_all(&bytes[inicio..fim]) {
        Ok(()) => dart_int((fim - inicio) as i64),
        Err(e) => erro_de_io(&e),
    }
}

/// `SynchronousSocket_ShutdownRead`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_ShutdownRead(this: i64) {
    if let Some(Some(s)) = soquete_sincrono(this) {
        let _ = s.shutdown(std::net::Shutdown::Read);
    }
}

/// `SynchronousSocket_ShutdownWrite`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SynchronousSocket_ShutdownWrite(this: i64) {
    if let Some(Some(s)) = soquete_sincrono(this) {
        let _ = s.shutdown(std::net::Shutdown::Write);
    }
}

// ---------------------------------------------------------------------------
// Mensagens de controle e recursos passados por soquete de domínio Unix
// (`SocketControlMessage`, `ResourceHandle`; N04). Os objetos Dart saem dos
// ajudantes `_dartforge*` de `sdk_nativo/io/common_patch.dart`; no Windows,
// como na VM, não há passagem de descritores.

/// A função Dart registrada `nome` (da sobreposição de `dart:io`).
#[cfg(unix)]
fn ajudante_de_io(nome: &str) -> usize {
    ajudante(nome).unwrap_or_else(|| panic!("bug do compilador: dart:io sem `{nome}` registrado"))
}

/// O `int` de um elemento de `List<dynamic>` (escalar ou caixa).
#[cfg(unix)]
fn int_da_lista(lista: i64, i: usize) -> Option<i64> {
    HEAP.with(|h| {
        let h = h.borrow();
        match h.lista_get(lista, i) {
            crate::heap::Valor::Int(v) => Some(v),
            crate::heap::Valor::Ref(r) => h.int_de(r),
            _ => None,
        }
    })
}

/// A `UnsupportedError` dos recursos por soquete fora do Unix.
#[cfg(windows)]
fn lancar_sem_recursos() {
    let msg = alocar_str("This is not supported on this operating system");
    let e = com_raizes(&[msg], || dartforge_unsupported_error_new(msg));
    com_raizes(&[e], || dartforge_exception_throw(e, 3));
}

/// `SocketControlMessage.fromHandles(handles)`: a mensagem `SCM_RIGHTS`
/// com os descritores.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SocketControlMessage_fromHandles(recursos: i64) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `(List<ResourceHandle>, int, int) → Object`.
        let f = { let alvo_dart: usize = ajudante_de_io("_dartforgeMensagemDeRecursos"); move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(alvo_dart, a0, a1, a2) } };
        let (nivel, tipo) = nivel_e_tipo_de_recursos();
        f(recursos, nivel, tipo)
    }
    #[cfg(windows)]
    {
        let _ = recursos;
        lancar_sem_recursos();
        0
    }
}

/// `_SocketControlMessageImpl.extractHandles()`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SocketControlMessageImpl_extractHandles(this: i64) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `(Object, int, int) → List<ResourceHandle>`.
        let f = { let alvo_dart: usize = ajudante_de_io("_dartforgeRecursosDaMensagem"); move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(alvo_dart, a0, a1, a2) } };
        let (nivel, tipo) = nivel_e_tipo_de_recursos();
        f(this, nivel, tipo)
    }
    #[cfg(windows)]
    {
        let _ = this;
        lancar_sem_recursos();
        0
    }
}

/// `Socket_SendMessage(buffer, offset, bytes, controlMessages)`: os bytes
/// enviados; `controlMessages` é `[nível, tipo, Uint8List]*`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_SendMessage(this: i64, buffer: i64, inicio: i64, n: i64, controles: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    #[cfg(unix)]
    {
        let bytes = bytes_da_lista_tipada(buffer).unwrap_or_default();
        let inicio = (inicio.max(0) as usize).min(bytes.len());
        let fim = (inicio + n.max(0) as usize).min(bytes.len());
        let total = HEAP.with(|h| h.borrow().lista_len(controles));
        let mut mensagens = Vec::with_capacity(total / 3);
        for i in (0..total - total % 3).step_by(3) {
            let dados = HEAP.with(|h| h.borrow().lista_get(controles, i + 2));
            let (dados_ref, dados_e_ref) = match dados {
                crate::heap::Valor::Ref(r) => (r, true),
                _ => (0, false),
            };
            match (int_da_lista(controles, i), int_da_lista(controles, i + 1), bytes_da_lista_tipada(dados_ref)) {
                (Some(nivel), Some(tipo), Some(d)) if dados_e_ref => mensagens.push((nivel, tipo, d)),
                _ => {
                    lancar_erro_de_argumento("Invalid control message");
                    return 0;
                }
            }
        }
        match enviar_mensagem(s.descritor(), &bytes[inicio..fim], &mensagens) {
            Ok(n) => dart_int(n as i64),
            Err(e) => {
                lancar_os_error(&e);
                0
            }
        }
    }
    #[cfg(windows)]
    {
        let _ = (s, buffer, inicio, n, controles);
        lancar_sem_recursos();
        0
    }
}

/// `Socket_ReceiveMessage(len)`: `[nível, tipo, Uint8List]*` e, por último,
/// a `Uint8List` dos dados.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Socket_ReceiveMessage(this: i64, n: i64) -> i64 {
    let Some(s) = soquete_do_objeto(this) else { return 0 };
    #[cfg(unix)]
    {
        let (dados, mensagens) = match receber_mensagem(s.descritor(), n.max(0) as usize) {
            Ok(x) => x,
            Err(e) => {
                lancar_os_error(&e);
                return 0;
            }
        };
        let mut itens: Vec<i64> = Vec::with_capacity(mensagens.len() * 3 + 1);
        for (nivel, tipo, d) in mensagens {
            let b = com_raizes(&itens, || dart_bytes(d));
            itens.extend([dart_int(nivel), dart_int(tipo), b]);
        }
        let b = com_raizes(&itens, || dart_bytes(dados));
        itens.push(b);
        com_raizes(&itens, || dart_lista_fixa(&itens))
    }
    #[cfg(windows)]
    {
        let _ = (s, n);
        lancar_sem_recursos();
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
        let descritor = { let alvo_dart: usize = ajudante_de_io("_dartforgeDescritorDoRecurso"); move |a0: i64| -> i64 { dart_r1(alvo_dart, a0) } };
        let arquivo = { let alvo_dart: usize = ajudante_de_io("_dartforgeArquivoDoRecurso"); move |a0: i64| -> i64 { dart_r1(alvo_dart, a0) } };
        arquivo(ArquivoNativo::novo(descritor(this)))
    }
    #[cfg(windows)]
    {
        let _ = this;
        lancar_sem_recursos();
        0
    }
}

/// `ResourceHandleImpl_toRawSocket`: `[tipo, texto, bytes, fd]` do endereço
/// local do descritor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ResourceHandleImpl_toRawSocket(this: i64) -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `(Object) → int`.
        let descritor = { let alvo_dart: usize = ajudante_de_io("_dartforgeDescritorDoRecurso"); move |a0: i64| -> i64 { dart_r1(alvo_dart, a0) } };
        let fd = descritor(this);
        match endereco_local(fd) {
            Ok(e) => {
                let tipo = e.tipo();
                let texto = e.texto();
                let bruto = if tipo == TIPO_UNIX { texto.clone().into_bytes() } else { e.ip() };
                let t = alocar_str(&texto);
                let b = com_raizes(&[t], || dart_bytes(bruto));
                com_raizes(&[t, b], || dart_lista_fixa(&[dart_int(tipo), t, b, dart_int(fd)]))
            }
            Err(e) => {
                lancar_os_error(&e);
                0
            }
        }
    }
    #[cfg(windows)]
    {
        let _ = this;
        lancar_sem_recursos();
        0
    }
}

/// `ResourceHandleImpl_toSocket` e `_toRawDatagramSocket`: a VM não os
/// suporta (o native dela devolve um `UnsupportedError`, que o retorno
/// tipado transforma em `TypeError`); aqui a `UnsupportedError` é lançada.
fn lancar_recurso_nao_suportado(api: &str) {
    let msg = alocar_str(&format!("ResourceHandle.{api} is not supported"));
    let e = com_raizes(&[msg], || dartforge_unsupported_error_new(msg));
    com_raizes(&[e], || dartforge_exception_throw(e, 3));
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ResourceHandleImpl_toSocket(_this: i64) -> i64 {
    lancar_recurso_nao_suportado("toSocket");
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ResourceHandleImpl_toRawDatagramSocket(_this: i64) -> i64 {
    lancar_recurso_nao_suportado("toRawDatagramSocket");
    0
}
