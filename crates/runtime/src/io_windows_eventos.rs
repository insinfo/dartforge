// Runtime nativo: o manipulador de eventos do `dart:io` no Windows
// (`runtime/bin/eventhandler_win.cc` da VM), sobre uma porta de conclusão
// de E/S (IOCP).
//
// No Windows o "descritor" de um `_NativeSocket` é um [`Manipulador`] (o
// `Handle` da VM): o `SOCKET`/`HANDLE` do sistema, as fichas e portas Dart
// ([`InfoDeDescritor`], o mesmo do Unix) e a E/S sobreposta em andamento.
// Não há prontidão para vigiar como no epoll: o manipulador mantém uma
// leitura emitida (`WSARecv`/`ReadFile`/`AcceptEx`) e guarda o que chegou;
// o Dart lê desse buffer (`Socket_Read`), e cada leitura que o esvazia
// emite a seguinte. A escrita copia os bytes e emite `WSASend`/`WriteFile`;
// enquanto ela não conclui o soquete está "cheio" (escreve 0). As
// conclusões e os comandos do Dart chegam pela mesma porta de conclusão e
// são atendidos por uma thread só, como na VM.
//
// A entrada padrão não aceita E/S sobreposta: uma thread faz o `ReadFile`
// síncrono e posta a conclusão na porta.
//
// Tudo o que é do Windows fica neste módulo; a raiz do runtime vê só os
// nomes que os natives comuns usam.

#[cfg(windows)]
mod es_windows {
    use super::*;
    use std::ffi::c_void;
    use std::sync::Arc;

    /// Um `HANDLE` ou `SOCKET` do sistema.
    pub(super) type Bruto = usize;
    /// `INVALID_HANDLE_VALUE` e `INVALID_SOCKET`.
    pub(super) const INVALIDO: Bruto = usize::MAX;

    pub(super) const ERROR_BROKEN_PIPE: u32 = 109;
    pub(super) const ERROR_IO_PENDING: u32 = 997;
    pub(super) const ERROR_IO_INCOMPLETE: u32 = 996;
    const ERROR_OPERATION_ABORTED: u32 = 995;
    const ERROR_NETNAME_DELETED: u32 = 64;
    const ERROR_CONNECTION_ABORTED: u32 = 1236;
    const ERROR_INVALID_USER_BUFFER: i32 = 1784;
    pub(super) const WSAECONNRESET: u32 = 10054;
    const FILE_TYPE_CHAR: u32 = 2;
    const INFINITE: u32 = u32::MAX;

    pub(super) const AF_INET: u16 = 2;
    pub(super) const AF_INET6: u16 = 23;
    pub(super) const SOCK_STREAM: i32 = 1;
    pub(super) const SOCK_DGRAM: i32 = 2;
    pub(super) const IPPROTO_IP: i32 = 0;
    pub(super) const IPPROTO_TCP: i32 = 6;
    pub(super) const IPPROTO_UDP: i32 = 17;
    pub(super) const IPPROTO_IPV6: i32 = 41;
    pub(super) const SOL_SOCKET: i32 = 0xffff;
    const SO_UPDATE_ACCEPT_CONTEXT: i32 = 0x700b;
    const SO_UPDATE_CONNECT_CONTEXT: i32 = 0x7010;
    const SD_RECEIVE: i32 = 0;
    const SD_SEND: i32 = 1;
    const SD_BOTH: i32 = 2;
    const WSA_FLAG_OVERLAPPED: u32 = 0x1;
    const WSA_FLAG_NO_HANDLE_INHERIT: u32 = 0x80;
    const SIO_GET_EXTENSION_FUNCTION_POINTER: u32 = 0xc800_0006;
    const TF_REUSE_SOCKET: u32 = 0x2;

    /// O tamanho do buffer de leitura (`kBufferSize`), que comporta um
    /// datagrama UDP inteiro.
    const TAMANHO_DO_BUFFER: usize = 64 * 1024;
    /// O que um `ReadFile` de console lê por vez (`kStdOverlappedBufferSize`).
    const TAMANHO_DE_LEITURA_DE_CONSOLE: usize = 16 * 1024;
    /// O maior datagrama UDP (`kMaxUDPPackageLength`).
    const TAMANHO_MAXIMO_DE_DATAGRAMA: usize = 64 * 1024;
    /// O espaço de cada endereço do `AcceptEx`: um `SOCKADDR_STORAGE` e
    /// mais 16 bytes (`kAcceptExAddressStorageSize`).
    const TAMANHO_DE_ENDERECO_DE_ACEITE: u32 = 128 + 16;
    /// Os `AcceptEx` que um soquete de escuta mantém emitidos
    /// (`kMinIssuedAccepts`).
    const ACEITES_EMITIDOS: usize = 5;

    /// A chave de conclusão dos comandos do Dart (o `InterruptMessage`); a
    /// da E/S é [`CHAVE_DE_ES`].
    const CHAVE_DE_COMANDO: usize = 0;
    const CHAVE_DE_ES: usize = 1;

    /// `OVERLAPPED`.
    #[repr(C)]
    #[derive(Default)]
    pub(super) struct Sobreposto {
        interno: usize,
        interno_alto: usize,
        deslocamento: u32,
        deslocamento_alto: u32,
        evento: usize,
    }

    impl Sobreposto {
        /// Um `OVERLAPPED` com o evento dado.
        pub(super) fn com_evento(evento: usize) -> Sobreposto {
            Sobreposto { evento, ..Sobreposto::default() }
        }
    }

    /// `WSABUF`.
    #[repr(C)]
    struct BufferWsa {
        tamanho: u32,
        dados: *mut u8,
    }

    /// `GUID`.
    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateIoCompletionPort(arquivo: Bruto, existente: usize, chave: usize, threads: u32) -> usize;
        fn GetQueuedCompletionStatus(porta: usize, bytes: *mut u32, chave: *mut usize, ov: *mut *mut Sobreposto, espera: u32) -> i32;
        fn PostQueuedCompletionStatus(porta: usize, bytes: u32, chave: usize, ov: *mut Sobreposto) -> i32;
        pub(super) fn CloseHandle(h: Bruto) -> i32;
        pub(super) fn ReadFile(h: Bruto, b: *mut u8, n: u32, lidos: *mut u32, ov: *mut Sobreposto) -> i32;
        pub(super) fn WriteFile(h: Bruto, b: *const u8, n: u32, escritos: *mut u32, ov: *mut Sobreposto) -> i32;
        fn CancelIoEx(h: Bruto, ov: *mut Sobreposto) -> i32;
        pub(super) fn GetLastError() -> u32;
        pub(super) fn GetFileType(h: Bruto) -> u32;
        fn GetStdHandle(qual: u32) -> Bruto;
    }

    #[link(name = "ws2_32")]
    unsafe extern "system" {
        fn WSAStartup(versao: u16, dados: *mut u8) -> i32;
        pub(super) fn WSAGetLastError() -> i32;
        pub(super) fn WSASetLastError(e: i32);
        fn WSASocketW(familia: i32, tipo: i32, protocolo: i32, info: *const c_void, grupo: u32, bandeiras: u32) -> Bruto;
        pub(super) fn closesocket(s: Bruto) -> i32;
        fn shutdown(s: Bruto, como: i32) -> i32;
        pub(super) fn bind(s: Bruto, e: *const u8, n: i32) -> i32;
        pub(super) fn listen(s: Bruto, fila: i32) -> i32;
        pub(super) fn setsockopt(s: Bruto, nivel: i32, opcao: i32, v: *const u8, n: i32) -> i32;
        pub(super) fn getsockopt(s: Bruto, nivel: i32, opcao: i32, v: *mut u8, n: *mut i32) -> i32;
        pub(super) fn getsockname(s: Bruto, e: *mut u8, n: *mut i32) -> i32;
        pub(super) fn getpeername(s: Bruto, e: *mut u8, n: *mut i32) -> i32;
        fn WSARecv(s: Bruto, b: *mut BufferWsa, n: u32, recebidos: *mut u32, bandeiras: *mut u32, ov: *mut Sobreposto, rotina: usize) -> i32;
        fn WSASend(s: Bruto, b: *mut BufferWsa, n: u32, enviados: *mut u32, bandeiras: u32, ov: *mut Sobreposto, rotina: usize) -> i32;
        fn WSARecvFrom(
            s: Bruto,
            b: *mut BufferWsa,
            n: u32,
            recebidos: *mut u32,
            bandeiras: *mut u32,
            de: *mut u8,
            de_n: *mut i32,
            ov: *mut Sobreposto,
            rotina: usize,
        ) -> i32;
        fn WSASendTo(s: Bruto, b: *mut BufferWsa, n: u32, enviados: *mut u32, bandeiras: u32, para: *const u8, para_n: i32, ov: *mut Sobreposto, rotina: usize) -> i32;
        fn WSAIoctl(s: Bruto, codigo: u32, ent: *const c_void, ent_n: u32, sai: *mut c_void, sai_n: u32, bytes: *mut u32, ov: *mut Sobreposto, rotina: usize) -> i32;
    }

    // -----------------------------------------------------------------------
    // Winsock e as extensões (`AcceptEx`, `ConnectEx`…).

    /// `SocketBase::Initialize`: o `WSAStartup`, uma vez por processo.
    pub(super) fn iniciar_winsock() {
        static INICIADO: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        INICIADO.get_or_init(|| {
            let mut dados = [0u8; 512];
            // SAFETY: o `WSADATA` cabe em 512 bytes.
            if unsafe { WSAStartup(0x0202, dados.as_mut_ptr()) } != 0 {
                panic!("dart:io: falha ao iniciar o Winsock ({})", unsafe { WSAGetLastError() });
            }
        });
    }

    type FnAcceptEx = unsafe extern "system" fn(Bruto, Bruto, *mut u8, u32, u32, u32, *mut u32, *mut Sobreposto) -> i32;
    type FnConnectEx = unsafe extern "system" fn(Bruto, *const u8, i32, *const u8, u32, *mut u32, *mut Sobreposto) -> i32;
    type FnDisconnectEx = unsafe extern "system" fn(Bruto, *mut Sobreposto, u32, u32) -> i32;
    type FnEnderecosDoAcceptEx = unsafe extern "system" fn(*mut u8, u32, u32, u32, *mut *mut u8, *mut i32, *mut *mut u8, *mut i32);

    /// As funções de extensão do Winsock (`InitializeSocketExtensions`).
    struct Extensoes {
        aceitar: FnAcceptEx,
        conectar: FnConnectEx,
        desconectar: FnDisconnectEx,
        enderecos_do_aceite: FnEnderecosDoAcceptEx,
    }

    fn extensoes() -> &'static Extensoes {
        static E: std::sync::OnceLock<Extensoes> = std::sync::OnceLock::new();
        E.get_or_init(|| {
            iniciar_winsock();
            // SAFETY: um soquete provisório só para consultar as extensões.
            let s = unsafe { WSASocketW(i32::from(AF_INET), SOCK_STREAM, IPPROTO_TCP, std::ptr::null(), 0, WSA_FLAG_OVERLAPPED) };
            if s == INVALIDO {
                panic!("dart:io: falha ao criar o soquete das extensões do Winsock");
            }
            let funcao = |guid: Guid| -> usize {
                let mut f = 0usize;
                let mut n = 0u32;
                // SAFETY: consulta o ponteiro da extensão `guid`.
                let r = unsafe {
                    WSAIoctl(
                        s,
                        SIO_GET_EXTENSION_FUNCTION_POINTER,
                        (&guid as *const Guid).cast(),
                        std::mem::size_of::<Guid>() as u32,
                        (&mut f as *mut usize).cast(),
                        std::mem::size_of::<usize>() as u32,
                        &mut n,
                        std::ptr::null_mut(),
                        0,
                    )
                };
                if r != 0 || f == 0 {
                    panic!("dart:io: extensão do Winsock indisponível");
                }
                f
            };
            // SAFETY: os ponteiros vieram do `WSAIoctl` com os GUIDs
            // (`WSAID_*`) de cada função, com estas assinaturas.
            let e = unsafe {
                Extensoes {
                    aceitar: std::mem::transmute::<usize, FnAcceptEx>(funcao(Guid(
                        0xb536_7df1,
                        0xcbac,
                        0x11cf,
                        [0x95, 0xca, 0x00, 0x80, 0x5f, 0x48, 0xa1, 0x92],
                    ))),
                    conectar: std::mem::transmute::<usize, FnConnectEx>(funcao(Guid(
                        0x25a2_07b9,
                        0xddf3,
                        0x4660,
                        [0x8e, 0xe9, 0x76, 0xe5, 0x8c, 0x74, 0x06, 0x3e],
                    ))),
                    desconectar: std::mem::transmute::<usize, FnDisconnectEx>(funcao(Guid(
                        0x7fda_2e11,
                        0x8630,
                        0x436f,
                        [0xa0, 0x31, 0xf5, 0x36, 0xa6, 0xee, 0xc1, 0x57],
                    ))),
                    enderecos_do_aceite: std::mem::transmute::<usize, FnEnderecosDoAcceptEx>(funcao(Guid(
                        0xb536_7df2,
                        0xcbac,
                        0x11cf,
                        [0x95, 0xca, 0x00, 0x80, 0x5f, 0x48, 0xa1, 0x92],
                    ))),
                }
            };
            // SAFETY: o soquete provisório não é mais usado.
            unsafe { closesocket(s) };
            e
        })
    }

    /// Um soquete sobreposto que os filhos não herdam.
    pub(super) fn novo_soquete(familia: u16, tipo: i32, protocolo: i32) -> ResultadoIo<Bruto> {
        iniciar_winsock();
        // SAFETY: cria um soquete.
        let s = unsafe {
            WSASocketW(i32::from(familia), tipo, protocolo, std::ptr::null(), 0, WSA_FLAG_OVERLAPPED | WSA_FLAG_NO_HANDLE_INHERIT)
        };
        if s == INVALIDO {
            return Err(ErroDoSo::do_codigo(unsafe { WSAGetLastError() }));
        }
        Ok(s)
    }

    // -----------------------------------------------------------------------
    // As operações sobrepostas (`OverlappedBuffer`).

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum TipoDeOperacao {
        Aceite,
        Leitura,
        Recepcao,
        Escrita,
        Envio,
        Desconexao,
        Conexao,
    }

    /// Uma operação em andamento: o `OVERLAPPED` (primeiro campo, para a
    /// conclusão achar a operação), o buffer e a referência do
    /// [`Manipulador`] que ela mantém viva até concluir.
    #[repr(C)]
    struct Operacao {
        sobreposto: Sobreposto,
        tipo: TipoDeOperacao,
        manipulador: Option<Arc<Manipulador>>,
        dados: Vec<u8>,
        /// O próximo byte a ler e quantos há (leituras).
        indice: usize,
        tamanho: usize,
        buffer_wsa: BufferWsa,
        bandeiras: u32,
        /// O endereço de origem (`WSARecvFrom`) ou de destino (`WSASendTo`).
        endereco: [u8; 128],
        tamanho_do_endereco: i32,
        /// O soquete que o `AcceptEx` preenche.
        cliente: Bruto,
    }

    impl Operacao {
        fn nova(tipo: TipoDeOperacao, m: &Arc<Manipulador>, capacidade: usize) -> Box<Operacao> {
            Box::new(Operacao {
                sobreposto: Sobreposto::default(),
                tipo,
                manipulador: Some(Arc::clone(m)),
                dados: vec![0; capacidade],
                indice: 0,
                tamanho: 0,
                buffer_wsa: BufferWsa { tamanho: 0, dados: std::ptr::null_mut() },
                bandeiras: 0,
                endereco: [0; 128],
                tamanho_do_endereco: 128,
                cliente: INVALIDO,
            })
        }

        fn sobreposto_limpo(&mut self) -> *mut Sobreposto {
            self.sobreposto = Sobreposto::default();
            &mut self.sobreposto
        }

        fn buffer_wsa(&mut self) -> *mut BufferWsa {
            self.buffer_wsa = BufferWsa { tamanho: self.dados.len() as u32, dados: self.dados.as_mut_ptr() };
            &mut self.buffer_wsa
        }

        fn restante(&self) -> usize {
            self.tamanho - self.indice
        }

        fn ler(&mut self, destino: &mut [u8]) -> usize {
            let n = destino.len().min(self.restante());
            destino[..n].copy_from_slice(&self.dados[self.indice..self.indice + n]);
            self.indice += n;
            n
        }

        /// Entrega a operação ao sistema, que a devolve na conclusão.
        fn emitida(self: Box<Self>) {
            let _ = Box::into_raw(self);
        }
    }

    // SAFETY: o único ponteiro da operação (`buffer_wsa.dados`) aponta para
    // o próprio `dados`; a operação passa entre threads inteira (a do Dart,
    // o sistema e a do manipulador), nunca compartilhada.
    unsafe impl Send for Operacao {}

    impl Drop for Operacao {
        fn drop(&mut self) {
            if self.cliente != INVALIDO {
                // SAFETY: o soquete do `AcceptEx` que ninguém adotou.
                unsafe { closesocket(self.cliente) };
            }
        }
    }

    // -----------------------------------------------------------------------
    // O `Handle`.

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum TipoDeManipulador {
        /// Um arquivo ou pipe sobreposto (os pipes dos processos e dos
        /// sinais).
        Arquivo,
        /// A entrada padrão, sem E/S sobreposta.
        Padrao,
        Cliente,
        Escuta,
        Datagrama,
    }

    /// O `Handle` da VM.
    pub(super) struct Manipulador {
        pub(super) tipo: TipoDeManipulador,
        pub(super) bruto: Bruto,
        sobreposto: bool,
        estado: std::sync::Mutex<Estado>,
        /// Acorda a thread de leitura síncrona (entrada padrão).
        pedido_de_leitura: std::sync::Condvar,
    }

    pub(super) struct Estado {
        /// `kClosing`, `kCloseRead`, `kCloseWrite`, `kError`.
        fechando: bool,
        leitura_fechada: bool,
        escrita_fechada: bool,
        erro: bool,
        /// O objeto do sistema ainda precisa ser fechado.
        aberto: bool,
        info: InfoDeDescritor,
        /// Os dados lidos que o Dart ainda não consumiu (`data_ready_`).
        pronto: Option<Box<Operacao>>,
        leitura_pendente: bool,
        escrita_pendente: bool,
        pub(super) ultimo_erro: u32,
        // Cliente.
        conectado: bool,
        desconectado: bool,
        pub(super) remoto: Option<EnderecoSo>,
        // Escuta.
        pub(super) familia: u16,
        aceites_pendentes: usize,
        aceitos: std::collections::VecDeque<Arc<Manipulador>>,
        // Entrada padrão: a leitura entregue à thread e se ela existe.
        leitura_sincrona: Option<Box<Operacao>>,
        thread_de_leitura: bool,
    }

    impl Estado {
        /// O handle passa a quem chama (`Process::Wait`): o manipulador não
        /// o fecha nem faz mais E/S nele.
        pub(super) fn tomar_handle(&mut self) {
            self.fechando = true;
            self.aberto = false;
        }
    }

    impl Manipulador {
        /// Cria o manipulador de `bruto`, associado à porta de conclusão
        /// quando faz E/S sobreposta.
        pub(super) fn novo(tipo: TipoDeManipulador, bruto: Bruto) -> Arc<Manipulador> {
            let sobreposto = tipo != TipoDeManipulador::Padrao;
            let m = Arc::new(Manipulador {
                tipo,
                bruto,
                sobreposto,
                estado: std::sync::Mutex::new(Estado {
                    fechando: false,
                    leitura_fechada: false,
                    escrita_fechada: false,
                    erro: false,
                    aberto: true,
                    info: InfoDeDescritor::novo(tipo == TipoDeManipulador::Escuta),
                    pronto: None,
                    leitura_pendente: false,
                    escrita_pendente: false,
                    ultimo_erro: 0,
                    conectado: false,
                    desconectado: false,
                    remoto: None,
                    familia: AF_INET,
                    aceites_pendentes: 0,
                    aceitos: std::collections::VecDeque::new(),
                    leitura_sincrona: None,
                    thread_de_leitura: false,
                }),
                pedido_de_leitura: std::sync::Condvar::new(),
            });
            if sobreposto {
                let porta = manipulador_de_eventos().porta;
                // SAFETY: associa um handle aberto à porta de conclusão.
                if unsafe { CreateIoCompletionPort(bruto, porta, CHAVE_DE_ES, 0) } == 0 {
                    panic!("dart:io: falha ao associar o handle à porta de conclusão ({})", unsafe { GetLastError() });
                }
            }
            m
        }

        pub(super) fn estado(&self) -> std::sync::MutexGuard<'_, Estado> {
            self.estado.lock().unwrap_or_else(|e| e.into_inner())
        }

        fn e_soquete(&self) -> bool {
            matches!(self.tipo, TipoDeManipulador::Cliente | TipoDeManipulador::Escuta | TipoDeManipulador::Datagrama)
        }
    }

    impl Drop for Manipulador {
        fn drop(&mut self) {
            let e = self.estado.get_mut().unwrap_or_else(|e| e.into_inner());
            if e.aberto {
                // SAFETY: o objeto do sistema é deste manipulador, que não
                // o fechou.
                unsafe {
                    if self.e_soquete() {
                        closesocket(self.bruto);
                    } else {
                        CloseHandle(self.bruto);
                    }
                }
            }
        }
    }

    /// O descritor Dart de um manipulador (a referência passa ao objeto).
    pub(super) fn descritor_de(m: Arc<Manipulador>) -> i64 {
        Arc::into_raw(m) as i64
    }

    /// O manipulador de um descritor, com uma referência nova.
    ///
    /// # Safety
    /// `d` veio de [`descritor_de`] e a referência dele ainda existe.
    pub(super) unsafe fn manipulador_retido(d: i64) -> Arc<Manipulador> {
        let p = d as *const Manipulador;
        // SAFETY: garantido por quem chama.
        unsafe {
            Arc::increment_strong_count(p);
            Arc::from_raw(p)
        }
    }

    /// `Socket::CloseFd` do Windows: solta a referência do objeto.
    pub(super) fn fechar_descritor_de_soquete(d: i64) {
        // SAFETY: a referência do objeto, solta uma vez (o descritor vira
        // -1 antes).
        drop(unsafe { Arc::from_raw(d as *const Manipulador) });
    }

    pub(super) fn reter_descritor(d: i64) -> i64 {
        // SAFETY: `d` é o descritor vivo de um soquete de escuta registrado.
        std::mem::forget(unsafe { manipulador_retido(d) });
        d
    }

    /// O manipulador de um descritor que o objeto Dart mantém vivo durante
    /// o native (`None` se o objeto já o soltou: -1).
    pub(super) fn manipulador_de(d: i64) -> Option<Arc<Manipulador>> {
        // SAFETY: um descritor não negativo é uma referência viva do objeto.
        (d >= 0).then(|| unsafe { manipulador_retido(d) })
    }

    /// O manipulador do soquete de um objeto Dart (`None` se fechado).
    pub(super) fn manipulador_do_soquete(s: &SoqueteNativo) -> Option<Arc<Manipulador>> {
        manipulador_de(s.descritor())
    }

    /// `ERROR_INVALID_HANDLE`: o soquete já foi fechado.
    pub(super) fn handle_invalido() -> ErroDoSo {
        ErroDoSo::do_codigo(6)
    }

    // -----------------------------------------------------------------------
    // Eventos para o Dart.

    const MASCARA_DE_ENTRADA: i64 = 1 << EVENTO_ENTRADA;
    const MASCARA_DE_SAIDA: i64 = 1 << EVENTO_SAIDA;

    /// `DispatchEventIfEnabled`.
    fn despachar(e: &mut Estado, evento: i64) -> bool {
        if e.info.mascara() & evento != 0 {
            let p = e.info.proxima_porta();
            postar_evento(p, evento);
            return true;
        }
        false
    }

    /// `HandleClosed`.
    fn avisar_fechamento(e: &mut Estado) {
        if !e.fechando {
            e.info.avisar_todas(1 << EVENTO_FECHAMENTO);
        }
    }

    /// `HandleError`.
    fn avisar_erro(e: &mut Estado, codigo: u32) {
        e.ultimo_erro = codigo;
        e.erro = true;
        if !e.fechando {
            e.info.avisar_todas(1 << EVENTO_ERRO);
        }
    }

    /// `HandleIssueError`: o fim do pipe ou a conexão desfeita é
    /// fechamento; o resto é erro.
    fn falha_ao_emitir(m: &Manipulador, e: &mut Estado, codigo: u32) {
        let fim = if m.e_soquete() { WSAECONNRESET } else { ERROR_BROKEN_PIPE };
        if codigo == fim {
            avisar_fechamento(e);
        } else {
            avisar_erro(e, codigo);
        }
    }

    /// `IsClosed` de cada tipo.
    fn esta_fechado(m: &Manipulador, e: &Estado) -> bool {
        match m.tipo {
            TipoDeManipulador::Escuta => e.fechando && e.aceites_pendentes == 0,
            TipoDeManipulador::Cliente => e.conectado && e.desconectado && !e.leitura_pendente && !e.escrita_pendente,
            _ => e.fechando && !e.leitura_pendente && !e.escrita_pendente,
        }
    }

    /// `NotifyDestroyedIfClosed`.
    fn avisar_destruicao_se_fechado(m: &Manipulador, e: &mut Estado) {
        if esta_fechado(m, e) {
            e.info.avisar_todas(1 << EVENTO_DESTRUIDO);
            e.info.remover_todas();
        }
    }

    // -----------------------------------------------------------------------
    // Emissão de E/S (com o estado travado).

    fn ultimo_erro_do_sistema() -> u32 {
        // SAFETY: lê o erro da thread.
        unsafe { GetLastError() }
    }

    /// `IssueReadLocked`: mantém uma leitura em andamento enquanto o Dart
    /// não tem dados para consumir.
    fn emitir_leitura(m: &Arc<Manipulador>, e: &mut Estado) -> bool {
        match m.tipo {
            TipoDeManipulador::Datagrama => return emitir_recepcao(m, e),
            TipoDeManipulador::Escuta => return false,
            _ => {}
        }
        if e.leitura_pendente || e.pronto.is_some() || e.fechando || e.leitura_fechada {
            return true;
        }
        let mut op = Operacao::nova(TipoDeOperacao::Leitura, m, TAMANHO_DO_BUFFER);
        e.leitura_pendente = true;
        if !m.sobreposto {
            // A thread de leitura faz o `ReadFile` síncrono e posta a
            // conclusão.
            e.leitura_sincrona = Some(op);
            if !e.thread_de_leitura {
                e.thread_de_leitura = true;
                let m2 = Arc::clone(m);
                std::thread::Builder::new()
                    .name("dart:io ReadFile".to_string())
                    .spawn(move || laco_de_leitura_sincrona(&m2))
                    .expect("dart:io: falha ao criar a thread de leitura");
            }
            m.pedido_de_leitura.notify_one();
            return true;
        }
        let ok = if m.tipo == TipoDeManipulador::Cliente {
            let b = op.buffer_wsa();
            let f: *mut u32 = &mut op.bandeiras;
            let ov = op.sobreposto_limpo();
            // SAFETY: a operação (buffer, bandeiras, `OVERLAPPED`) vive
            // até a conclusão.
            unsafe { WSARecv(m.bruto, b, 1, std::ptr::null_mut(), f, ov, 0) == 0 }
        } else {
            let (p, n) = (op.dados.as_mut_ptr(), op.dados.len() as u32);
            let ov = op.sobreposto_limpo();
            // SAFETY: como acima.
            unsafe { ReadFile(m.bruto, p, n, std::ptr::null_mut(), ov) != 0 }
        };
        let codigo = ultimo_erro_do_sistema();
        if ok || codigo == ERROR_IO_PENDING {
            op.emitida();
            return true;
        }
        e.leitura_pendente = false;
        drop(op);
        falha_ao_emitir(m, e, codigo);
        false
    }

    /// `DatagramSocket::IssueRecvFromLocked`.
    fn emitir_recepcao(m: &Arc<Manipulador>, e: &mut Estado) -> bool {
        if e.leitura_pendente || e.pronto.is_some() || e.fechando || e.leitura_fechada {
            return true;
        }
        let mut op = Operacao::nova(TipoDeOperacao::Recepcao, m, TAMANHO_MAXIMO_DE_DATAGRAMA);
        let b = op.buffer_wsa();
        let f: *mut u32 = &mut op.bandeiras;
        let de = op.endereco.as_mut_ptr();
        let de_n: *mut i32 = &mut op.tamanho_do_endereco;
        let ov = op.sobreposto_limpo();
        e.leitura_pendente = true;
        // SAFETY: a operação vive até a conclusão.
        let ok = unsafe { WSARecvFrom(m.bruto, b, 1, std::ptr::null_mut(), f, de, de_n, ov, 0) == 0 };
        let codigo = ultimo_erro_do_sistema();
        if ok || codigo == ERROR_IO_PENDING {
            op.emitida();
            return true;
        }
        e.leitura_pendente = false;
        drop(op);
        falha_ao_emitir(m, e, codigo);
        false
    }

    /// `ListenSocket::IssueAcceptLocked`.
    fn emitir_aceite(m: &Arc<Manipulador>, e: &mut Estado) -> bool {
        let cliente = match novo_soquete(e.familia, SOCK_STREAM, IPPROTO_TCP) {
            Ok(s) => s,
            Err(erro) => {
                avisar_erro(e, erro.codigo as u32);
                return false;
            }
        };
        let mut op = Operacao::nova(TipoDeOperacao::Aceite, m, 2 * TAMANHO_DE_ENDERECO_DE_ACEITE as usize);
        op.cliente = cliente;
        let p = op.dados.as_mut_ptr();
        let ov = op.sobreposto_limpo();
        let mut recebidos = 0u32;
        // SAFETY: a operação (buffer dos endereços, `OVERLAPPED`) vive até
        // a conclusão.
        let ok = unsafe {
            (extensoes().aceitar)(m.bruto, cliente, p, 0, TAMANHO_DE_ENDERECO_DE_ACEITE, TAMANHO_DE_ENDERECO_DE_ACEITE, &mut recebidos, ov) != 0
        };
        let codigo = ultimo_erro_do_sistema();
        if ok || codigo == ERROR_IO_PENDING {
            e.aceites_pendentes += 1;
            op.emitida();
            return true;
        }
        drop(op);
        avisar_erro(e, codigo);
        false
    }

    /// O `ConnectEx` de `Socket::CreateConnect`: conclui pela porta, mesmo
    /// quando termina de imediato.
    pub(super) fn iniciar_conexao(m: &Arc<Manipulador>, destino: &EnderecoSo) -> ResultadoIo<()> {
        let mut op = Operacao::nova(TipoDeOperacao::Conexao, m, 0);
        let ov = op.sobreposto_limpo();
        // SAFETY: `destino` é um `sockaddr` válido; a operação vive até a
        // conclusão.
        let ok = unsafe {
            (extensoes().conectar)(m.bruto, destino.bytes.as_ptr(), destino.tamanho_da_familia() as i32, std::ptr::null(), 0, std::ptr::null_mut(), ov) != 0
        };
        let codigo = ultimo_erro_do_sistema();
        if ok || codigo == ERROR_IO_PENDING {
            op.emitida();
            return Ok(());
        }
        drop(op);
        Err(ErroDoSo::do_codigo(codigo as i32))
    }

    /// `ListenSocket::StartAccept`: `false` com o erro em `ultimo_erro`.
    pub(super) fn comecar_a_aceitar(m: &Arc<Manipulador>) -> bool {
        let mut e = m.estado();
        (0..ACEITES_EMITIDOS).all(|_| emitir_aceite(m, &mut e))
    }

    /// `ClientSocket::IssueDisconnectLocked`: a desconexão e o aviso de
    /// destruição às portas.
    fn emitir_desconexao(m: &Arc<Manipulador>, e: &mut Estado) {
        let mut op = Operacao::nova(TipoDeOperacao::Desconexao, m, 0);
        let ov = op.sobreposto_limpo();
        // SAFETY: a operação vive até a conclusão.
        let ok = unsafe { (extensoes().desconectar)(m.bruto, ov, TF_REUSE_SOCKET, 0) != 0 };
        let codigo = ultimo_erro_do_sistema();
        if ok || codigo == ERROR_IO_PENDING {
            // Mesmo concluída de imediato, a conclusão passa pela porta.
            op.emitida();
        } else {
            drop(op);
            concluir_desconexao(m, e);
        }
        e.info.avisar_todas(1 << EVENTO_DESTRUIDO);
        e.info.remover_todas();
    }

    /// `ClientSocket::DisconnectComplete`.
    fn concluir_desconexao(m: &Manipulador, e: &mut Estado) {
        if e.aberto {
            // SAFETY: o soquete é deste manipulador.
            unsafe { closesocket(m.bruto) };
            e.aberto = false;
        }
        e.pronto = None;
        e.desconectado = true;
    }

    /// `ClientSocket::ConnectComplete`.
    fn concluir_conexao(m: &Arc<Manipulador>, e: &mut Estado) {
        // SAFETY: atualiza o contexto do soquete conectado pelo
        // `ConnectEx`.
        unsafe { setsockopt(m.bruto, SOL_SOCKET, SO_UPDATE_CONNECT_CONTEXT, std::ptr::null(), 0) };
        if !e.leitura_fechada && e.info.mascara() & MASCARA_DE_ENTRADA != 0 {
            emitir_leitura(m, e);
        }
        if !e.escrita_fechada {
            despachar(e, MASCARA_DE_SAIDA);
        }
    }

    /// `ListenSocket::DispatchCompletedAcceptsLocked`.
    fn despachar_aceites(e: &mut Estado) {
        if e.fechando {
            return;
        }
        for _ in 0..e.aceitos.len() {
            if !despachar(e, MASCARA_DE_ENTRADA) {
                break;
            }
        }
    }

    /// `Handle::CloseLocked`.
    fn fechar_manipulador(m: &Arc<Manipulador>, e: &mut Estado) {
        if !m.sobreposto && e.aberto {
            // Destrava o `ReadFile` síncrono da thread de leitura.
            // SAFETY: cancela a E/S do handle.
            unsafe { CancelIoEx(m.bruto, std::ptr::null_mut()) };
        }
        if e.fechando {
            return;
        }
        e.fechando = true;
        match m.tipo {
            TipoDeManipulador::Arquivo | TipoDeManipulador::Padrao => {
                if e.aberto {
                    // SAFETY: o handle é deste manipulador; a E/S pendente
                    // conclui com `ERROR_OPERATION_ABORTED`.
                    unsafe { CloseHandle(m.bruto) };
                    e.aberto = false;
                }
                if m.tipo == TipoDeManipulador::Padrao {
                    m.pedido_de_leitura.notify_all();
                    let mut g = entrada_padrao();
                    if g.as_ref().is_some_and(|x| std::ptr::eq(Arc::as_ptr(x), Arc::as_ptr(m))) {
                        *g = None;
                    }
                }
            }
            TipoDeManipulador::Escuta => {
                if e.aberto {
                    // SAFETY: os `AcceptEx` pendentes concluem abortados.
                    unsafe { closesocket(m.bruto) };
                    e.aberto = false;
                }
                for c in std::mem::take(&mut e.aceitos) {
                    let mut ce = c.estado();
                    fechar_manipulador(&c, &mut ce);
                    avisar_destruicao_se_fechado(&c, &mut ce);
                }
            }
            TipoDeManipulador::Cliente => {
                // SAFETY: encerra os dois sentidos antes de desconectar.
                unsafe { shutdown(m.bruto, SD_BOTH) };
                emitir_desconexao(m, e);
            }
            TipoDeManipulador::Datagrama => {
                if e.aberto {
                    // SAFETY: a E/S pendente conclui abortada.
                    unsafe { closesocket(m.bruto) };
                    e.aberto = false;
                }
                e.leitura_fechada = true;
                e.escrita_fechada = true;
            }
        }
    }

    /// Fecha um manipulador que nenhum objeto Dart chegou a usar.
    pub(super) fn descartar(m: &Arc<Manipulador>) {
        let mut e = m.estado();
        match m.tipo {
            TipoDeManipulador::Cliente => {
                // Nunca conectou: fecha já, sem `DisconnectEx`.
                e.fechando = true;
                concluir_desconexao(m, &mut e);
            }
            _ => fechar_manipulador(m, &mut e),
        }
    }

    /// A thread da leitura síncrona (`Handle::ReadSyncCompleteAsync`):
    /// atende um pedido por vez e posta a conclusão na porta.
    fn laco_de_leitura_sincrona(m: &Arc<Manipulador>) {
        let porta = manipulador_de_eventos().porta;
        loop {
            let mut op = {
                let mut e = m.estado();
                loop {
                    if let Some(op) = e.leitura_sincrona.take() {
                        break op;
                    }
                    if e.fechando {
                        e.thread_de_leitura = false;
                        return;
                    }
                    e = m.pedido_de_leitura.wait(e).unwrap_or_else(|x| x.into_inner());
                }
            };
            // SAFETY: consulta o tipo do handle.
            let n = if unsafe { GetFileType(m.bruto) } == FILE_TYPE_CHAR { TAMANHO_DE_LEITURA_DE_CONSOLE } else { op.dados.len() };
            let mut lidos = 0u32;
            // SAFETY: leitura síncrona no buffer da operação.
            if unsafe { ReadFile(m.bruto, op.dados.as_mut_ptr(), n as u32, &mut lidos, std::ptr::null_mut()) } == 0 {
                lidos = 0;
            }
            let ov = op.sobreposto_limpo();
            op.emitida();
            // SAFETY: a operação volta pela porta, como uma sobreposta.
            if unsafe { PostQueuedCompletionStatus(porta, lidos, CHAVE_DE_ES, ov) } == 0 {
                panic!("dart:io: PostQueuedCompletionStatus falhou ({})", unsafe { GetLastError() });
            }
        }
    }

    // -----------------------------------------------------------------------
    // A entrada padrão.

    fn entrada_padrao() -> std::sync::MutexGuard<'static, Option<Arc<Manipulador>>> {
        static E: std::sync::Mutex<Option<Arc<Manipulador>>> = std::sync::Mutex::new(None);
        E.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `SocketBase::GetStdioHandle`: só a entrada (`StdHandle::Stdin`, um
    /// por processo).
    pub(super) fn manipulador_padrao(num: i64) -> i64 {
        if num != 0 {
            return -1;
        }
        // SAFETY: lê o handle do processo (STD_INPUT_HANDLE = -10).
        let h = unsafe { GetStdHandle(-10i32 as u32) };
        if h == INVALIDO || h == 0 {
            return -1;
        }
        let mut g = entrada_padrao();
        let m = Arc::clone(g.get_or_insert_with(|| Manipulador::novo(TipoDeManipulador::Padrao, h)));
        descritor_de(m)
    }

    // -----------------------------------------------------------------------
    // As operações que os natives fazem (`Handle::Read`, `Write`…).

    /// `Handle::Available`.
    pub(super) fn disponivel(s: &SoqueteNativo) -> i64 {
        manipulador_do_soquete(s).map_or(0, |m| m.estado().pronto.as_ref().map_or(0, |p| p.restante() as i64))
    }

    /// `Handle::Read`: o que já chegou; esvaziar o buffer emite a próxima
    /// leitura.
    pub(super) fn ler_de(s: &SoqueteNativo, destino: &mut [u8]) -> ResultadoIo<usize> {
        let Some(m) = manipulador_do_soquete(s) else { return Ok(0) };
        let mut e = m.estado();
        let Some(p) = e.pronto.as_mut() else { return Ok(0) };
        let n = p.ler(destino);
        if p.restante() == 0 {
            e.pronto = None;
            if !e.fechando && !e.leitura_fechada {
                emitir_leitura(&m, &mut e);
            }
        }
        Ok(n)
    }

    /// `Handle::Write`: copia até 64 KiB e emite a escrita; 0 enquanto
    /// outra está em andamento.
    pub(super) fn escrever_no_manipulador(m: &Arc<Manipulador>, dados: &[u8]) -> ResultadoIo<usize> {
        let mut e = m.estado();
        if e.escrita_pendente || esta_fechado(m, &e) {
            return Ok(0);
        }
        let n = dados.len().min(TAMANHO_DO_BUFFER);
        if !m.sobreposto {
            // A entrada padrão não é escrita pelo Dart; um handle síncrono
            // escreve na hora.
            let mut escritos = 0u32;
            // SAFETY: escrita síncrona dos bytes dados.
            if unsafe { WriteFile(m.bruto, dados.as_ptr(), n as u32, &mut escritos, std::ptr::null_mut()) } == 0 {
                return Err(ErroDoSo::do_codigo(ultimo_erro_do_sistema() as i32));
            }
            return Ok(escritos as usize);
        }
        let mut op = Operacao::nova(TipoDeOperacao::Escrita, m, 0);
        op.dados = dados[..n].to_vec();
        op.tamanho = n;
        e.escrita_pendente = true;
        let ok = if m.tipo == TipoDeManipulador::Cliente {
            let b = op.buffer_wsa();
            let ov = op.sobreposto_limpo();
            // SAFETY: a operação vive até a conclusão.
            unsafe { WSASend(m.bruto, b, 1, std::ptr::null_mut(), 0, ov, 0) == 0 }
        } else {
            let p = op.dados.as_ptr();
            let ov = op.sobreposto_limpo();
            // SAFETY: como acima.
            unsafe { WriteFile(m.bruto, p, n as u32, std::ptr::null_mut(), ov) != 0 }
        };
        let codigo = ultimo_erro_do_sistema();
        if ok || codigo == ERROR_IO_PENDING {
            op.emitida();
            return Ok(n);
        }
        e.escrita_pendente = false;
        drop(op);
        falha_ao_emitir(m, &mut e, codigo);
        Err(ErroDoSo::do_codigo(codigo as i32))
    }

    pub(super) fn escrever_em(s: &SoqueteNativo, dados: &[u8]) -> ResultadoIo<usize> {
        match manipulador_do_soquete(s) {
            Some(m) => escrever_no_manipulador(&m, dados),
            None => Ok(0),
        }
    }

    /// `SocketBase::HasPendingWrite`.
    pub(super) fn escrita_pendente(s: &SoqueteNativo) -> bool {
        manipulador_do_soquete(s).is_some_and(|m| m.estado().escrita_pendente)
    }

    /// `ServerSocket::Accept`: a próxima conexão aceita (o descritor dela)
    /// ou -1; repõe os `AcceptEx` emitidos.
    pub(super) fn aceitar(fd: i64) -> i64 {
        let Some(m) = manipulador_de(fd) else { return -1 };
        let mut e = m.estado();
        let r = e.aceitos.pop_front();
        if !e.fechando && e.aceites_pendentes < ACEITES_EMITIDOS {
            emitir_aceite(&m, &mut e);
        }
        r.map_or(-1, descritor_de)
    }

    /// `SocketBase::AvailableDatagram`.
    pub(super) fn ha_datagrama(fd: i64) -> bool {
        manipulador_de(fd).is_some_and(|m| m.estado().pronto.is_some())
    }

    /// `Handle::RecvFrom`: o datagrama recebido (o buffer inteiro sai, como
    /// o `recvfrom`) e a origem.
    pub(super) fn receber_datagrama(fd: i64, maximo: usize) -> ResultadoIo<Option<(Vec<u8>, EnderecoSo)>> {
        let Some(m) = manipulador_de(fd) else { return Ok(None) };
        let mut e = m.estado();
        let Some(op) = e.pronto.take() else { return Ok(None) };
        let n = op.restante().min(maximo);
        let dados = op.dados[op.indice..op.indice + n].to_vec();
        let origem = EnderecoSo::de_ponteiro(op.endereco.as_ptr(), op.tamanho_do_endereco.max(0) as u32);
        drop(op);
        if !e.fechando && !e.leitura_fechada {
            emitir_recepcao(&m, &mut e);
        }
        Ok(Some((dados, origem)))
    }

    /// `Handle::SendTo`.
    pub(super) fn enviar_datagrama(fd: i64, dados: &[u8], destino: &EnderecoSo) -> ResultadoIo<usize> {
        let Some(m) = manipulador_de(fd) else { return Err(handle_invalido()) };
        let mut e = m.estado();
        if e.escrita_pendente || esta_fechado(&m, &e) {
            return Ok(0);
        }
        if dados.len() > TAMANHO_MAXIMO_DE_DATAGRAMA {
            return Err(ErroDoSo::do_codigo(ERROR_INVALID_USER_BUFFER));
        }
        let mut op = Operacao::nova(TipoDeOperacao::Envio, &m, 0);
        op.dados = dados.to_vec();
        op.tamanho = dados.len();
        let n_destino = destino.tamanho_da_familia() as usize;
        op.endereco[..n_destino].copy_from_slice(&destino.bytes[..n_destino]);
        op.tamanho_do_endereco = n_destino as i32;
        let b = op.buffer_wsa();
        let para = op.endereco.as_ptr();
        let ov = op.sobreposto_limpo();
        e.escrita_pendente = true;
        // SAFETY: a operação (dados, endereço, `OVERLAPPED`) vive até a
        // conclusão.
        let ok = unsafe { WSASendTo(m.bruto, b, 1, std::ptr::null_mut(), 0, para, n_destino as i32, ov, 0) == 0 };
        let codigo = ultimo_erro_do_sistema();
        if ok || codigo == ERROR_IO_PENDING {
            op.emitida();
            return Ok(dados.len());
        }
        e.escrita_pendente = false;
        drop(op);
        falha_ao_emitir(&m, &mut e, codigo);
        Err(ErroDoSo::do_codigo(codigo as i32))
    }

    // -----------------------------------------------------------------------
    // A thread do manipulador.

    pub(super) fn iniciar_manipulador() -> ManipuladorDeEventos {
        // SAFETY: cria a porta de conclusão (uma thread concorrente).
        let porta = unsafe { CreateIoCompletionPort(INVALIDO, 0, 0, 1) };
        if porta == 0 {
            panic!("dart:io: falha ao criar a porta de conclusão ({})", unsafe { GetLastError() });
        }
        std::thread::Builder::new()
            .name("dart:io EventHandler".to_string())
            .spawn(move || laco_de_eventos(porta))
            .expect("dart:io: falha ao criar a thread do manipulador de eventos");
        ManipuladorDeEventos { porta }
    }

    impl ManipuladorDeEventos {
        /// `SendData`: o comando vai pela porta de conclusão.
        pub(super) fn enviar(&self, c: ComandoDeEvento) {
            let p = Box::into_raw(Box::new(c));
            // SAFETY: o laço recebe o ponteiro com a chave de comando e o
            // retoma.
            if unsafe { PostQueuedCompletionStatus(self.porta, 0, CHAVE_DE_COMANDO, p.cast()) } == 0 {
                panic!("dart:io: PostQueuedCompletionStatus falhou ({})", unsafe { GetLastError() });
            }
        }
    }

    fn laco_de_eventos(porta: usize) {
        loop {
            let mut bytes = 0u32;
            let mut chave = 0usize;
            let mut ov: *mut Sobreposto = std::ptr::null_mut();
            // SAFETY: espera uma conclusão na porta.
            let ok = unsafe { GetQueuedCompletionStatus(porta, &mut bytes, &mut chave, &mut ov, INFINITE) } != 0;
            if ov.is_null() {
                continue;
            }
            if chave == CHAVE_DE_COMANDO {
                // SAFETY: postado por `enviar` com um `ComandoDeEvento`.
                let c = unsafe { Box::from_raw(ov.cast::<ComandoDeEvento>()) };
                tratar_comando(*c);
            } else {
                let erro = if ok { 0 } else { ultimo_erro_do_sistema() };
                // SAFETY: o `OVERLAPPED` é o primeiro campo de uma
                // `Operacao` emitida.
                let op = unsafe { Box::from_raw(ov.cast::<Operacao>()) };
                tratar_conclusao(op, ok, bytes, erro);
            }
        }
    }

    /// `HandleCompletionOrInterrupt` e `HandleIOCompletion`.
    fn tratar_conclusao(mut op: Box<Operacao>, ok: bool, bytes: u32, erro: u32) {
        let Some(m) = op.manipulador.take() else { return };
        // O fechamento da conexão ou do handle, e as operações abortadas
        // pelo fechamento, contam como 0 bytes; o resto é erro (-1).
        let bytes: i64 = if ok {
            i64::from(bytes)
        } else if [ERROR_CONNECTION_ABORTED, ERROR_OPERATION_ABORTED, ERROR_NETNAME_DELETED, ERROR_BROKEN_PIPE].contains(&erro) {
            0
        } else {
            -1
        };
        let mut e = m.estado();
        match op.tipo {
            TipoDeOperacao::Aceite => concluir_aceite(&m, &mut e, op),
            TipoDeOperacao::Leitura => {
                op.tamanho = bytes.max(0) as usize;
                op.indice = 0;
                e.leitura_pendente = false;
                if !e.fechando {
                    e.pronto = Some(op);
                }
                if bytes > 0 {
                    if !e.fechando {
                        despachar(&mut e, MASCARA_DE_ENTRADA);
                    }
                } else {
                    e.leitura_fechada = true;
                    if bytes == 0 {
                        avisar_fechamento(&mut e);
                    } else {
                        avisar_erro(&mut e, erro);
                    }
                }
            }
            TipoDeOperacao::Recepcao => {
                e.leitura_pendente = false;
                if bytes >= 0 {
                    op.tamanho = bytes as usize;
                    op.indice = 0;
                    if !e.fechando {
                        e.pronto = Some(op);
                        despachar(&mut e, MASCARA_DE_ENTRADA);
                    }
                } else {
                    avisar_erro(&mut e, erro);
                }
            }
            TipoDeOperacao::Escrita | TipoDeOperacao::Envio => {
                e.escrita_pendente = false;
                if bytes >= 0 {
                    if !e.erro && !e.fechando {
                        despachar(&mut e, MASCARA_DE_SAIDA);
                    }
                } else {
                    avisar_erro(&mut e, erro);
                }
            }
            TipoDeOperacao::Desconexao => concluir_desconexao(&m, &mut e),
            TipoDeOperacao::Conexao => {
                if bytes < 0 {
                    avisar_erro(&mut e, erro);
                } else {
                    concluir_conexao(&m, &mut e);
                }
                e.conectado = true;
            }
        }
        avisar_destruicao_se_fechado(&m, &mut e);
    }

    /// `ListenSocket::AcceptComplete`: a conexão aceita entra na fila e o
    /// Dart é avisado.
    fn concluir_aceite(m: &Arc<Manipulador>, e: &mut Estado, mut op: Box<Operacao>) {
        if !e.fechando {
            let escuta = m.bruto;
            // SAFETY: herda o contexto do soquete de escuta.
            let r = unsafe {
                setsockopt(op.cliente, SOL_SOCKET, SO_UPDATE_ACCEPT_CONTEXT, (&escuta as *const Bruto).cast(), std::mem::size_of::<Bruto>() as i32)
            };
            if r == 0 {
                let cliente = std::mem::replace(&mut op.cliente, INVALIDO);
                // O `getpeername` não vale para um soquete aceito com E/S
                // sobreposta: o endereço remoto vem do `AcceptEx`.
                let (mut local, mut remoto) = (std::ptr::null_mut(), std::ptr::null_mut());
                let (mut n_local, mut n_remoto) = (0i32, 0i32);
                // SAFETY: o buffer é o que o `AcceptEx` preencheu.
                unsafe {
                    (extensoes().enderecos_do_aceite)(
                        op.dados.as_mut_ptr(),
                        0,
                        TAMANHO_DE_ENDERECO_DE_ACEITE,
                        TAMANHO_DE_ENDERECO_DE_ACEITE,
                        &mut local,
                        &mut n_local,
                        &mut remoto,
                        &mut n_remoto,
                    )
                };
                let c = Manipulador::novo(TipoDeManipulador::Cliente, cliente);
                {
                    let mut ce = c.estado();
                    ce.conectado = true;
                    if !remoto.is_null() {
                        ce.remoto = Some(EnderecoSo::de_ponteiro(remoto, n_remoto.max(0) as u32));
                    }
                }
                e.aceitos.push_back(c);
            }
        }
        e.aceites_pendentes -= 1;
        despachar_aceites(e);
    }

    /// `HandleInterrupt`: um comando do Dart para um soquete.
    fn tratar_comando(c: ComandoDeEvento) {
        // O comando carrega uma referência do soquete (solta ao fim).
        // SAFETY: `c.soquete` veio de `SoqueteNativo::novo`, retido para o
        // comando.
        let soquete = unsafe { Arc::from_raw(c.soquete as *const SoqueteNativo) };
        let fd = soquete.descritor();
        if fd < 0 {
            return;
        }
        if e_comando(c.dados, COMANDO_FECHAR) && c.dados & (1 << SOQUETE_DE_SINAL) != 0 {
            limpar_sinal_por_descritor(fd);
        }
        // SAFETY: o objeto mantém a referência enquanto o descritor vale.
        let m = unsafe { manipulador_retido(fd) };
        let mut e = m.estado();
        if e_comando(c.dados, COMANDO_DEVOLVER_FICHAS) {
            e.info.devolver_fichas(c.porta, c.dados & ((1 << COMANDO_FECHAR) - 1));
        } else if e_comando(c.dados, COMANDO_MASCARA) {
            let eventos = c.dados & MASCARA_DE_EVENTOS;
            e.info.definir_porta_e_mascara(c.porta, eventos);
            if m.tipo == TipoDeManipulador::Escuta {
                despachar_aceites(&mut e);
            } else {
                if e.info.mascara() & MASCARA_DE_ENTRADA != 0 && (m.tipo != TipoDeManipulador::Cliente || e.conectado) {
                    emitir_leitura(&m, &mut e);
                }
                // Pronto para escrever se nada está em andamento (o cliente,
                // depois de conectado).
                if eventos & MASCARA_DE_SAIDA != 0 && !e.escrita_pendente && (m.tipo != TipoDeManipulador::Cliente || e.conectado) {
                    despachar(&mut e, MASCARA_DE_SAIDA);
                }
                let ha_dados = e.pronto.as_ref().is_some_and(|p| p.restante() > 0 || m.tipo == TipoDeManipulador::Datagrama);
                if eventos & MASCARA_DE_ENTRADA != 0 && ha_dados {
                    despachar(&mut e, MASCARA_DE_ENTRADA);
                }
            }
        } else if e_comando(c.dados, COMANDO_FECHAR_LEITURA) {
            if m.tipo == TipoDeManipulador::Cliente {
                // SAFETY: soquete aberto deste manipulador.
                unsafe { shutdown(m.bruto, SD_RECEIVE) };
                e.leitura_fechada = true;
            }
        } else if e_comando(c.dados, COMANDO_FECHAR_ESCRITA) {
            if m.tipo == TipoDeManipulador::Cliente {
                // SAFETY: soquete aberto deste manipulador.
                unsafe { shutdown(m.bruto, SD_SEND) };
                e.escrita_fechada = true;
            }
        } else if e_comando(c.dados, COMANDO_FECHAR) {
            let mut fechar = true;
            if m.tipo == TipoDeManipulador::Escuta && !registro_de_escuta().fechar(c.soquete) {
                // Outros objetos escutam no mesmo soquete do sistema: este
                // sai, e o soquete fica.
                fechar = false;
                e.info.remover_porta(c.porta);
                postar_evento(c.porta, 1 << EVENTO_DESTRUIDO);
                soquete.soltar_descritor();
            }
            if fechar {
                // A porta recebe o aviso de destruição.
                e.info.definir_porta_e_mascara(c.porta, 0);
                fechar_manipulador(&m, &mut e);
                soquete.soltar_descritor();
            }
        }
        avisar_destruicao_se_fechado(&m, &mut e);
    }
}

#[cfg(windows)]
use es_windows::*;
