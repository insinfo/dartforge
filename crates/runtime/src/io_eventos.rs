// Runtime nativo: o manipulador de eventos do `dart:io` da VM
// (`runtime/bin/eventhandler*.cc`) e o objeto de soquete (`bin/socket.h`).
//
// Um `_NativeSocket` do Dart (soquete TCP, pipe de processo, entrada padrão)
// guarda no campo nativo um [`SoqueteNativo`]: o descritor e a porta Dart
// que recebe os eventos dele, com contagem de referências. O Dart pede ao
// manipulador, por `EventHandler_SendData`, para vigiar o descritor (uma
// máscara de leitura/escrita), devolver fichas ou fechar; o manipulador é
// uma thread com epoll (Linux), kqueue (macOS) ou uma porta de conclusão de
// E/S (Windows, em `io_windows_eventos.rs`) que posta na porta do
// soquete a máscara dos eventos prontos (`kInEvent`, `kOutEvent`,
// `kCloseEvent`, `kErrorEvent`, `kDestroyedEvent`) — como a VM, inclusive
// o controle de fluxo por fichas (`TokenCounter`) e os soquetes de escuta
// compartilhados entre portas (`DescriptorInfoMultiple`).
//
// Os timers do Dart não passam por aqui: o laço de eventos do isolado os
// atende (`eventos.rs`), e a sobreposição de `dart:async` não usa o
// `_EventHandler` para eles.

/// `MessageFlags` de `eventhandler.h`.
const EVENTO_ENTRADA: i64 = 0;
const EVENTO_SAIDA: i64 = 1;
const EVENTO_ERRO: i64 = 2;
const EVENTO_FECHAMENTO: i64 = 3;
const EVENTO_DESTRUIDO: i64 = 4;
const COMANDO_FECHAR: i64 = 8;
const COMANDO_FECHAR_LEITURA: i64 = 9;
const COMANDO_FECHAR_ESCRITA: i64 = 10;
const COMANDO_DEVOLVER_FICHAS: i64 = 11;
const COMANDO_MASCARA: i64 = 12;
const SOQUETE_DE_ESCUTA: i64 = 16;
const SOQUETE_DE_SINAL: i64 = 18;

const MASCARA_DE_COMANDOS: i64 = (1 << COMANDO_FECHAR)
    | (1 << COMANDO_FECHAR_LEITURA)
    | (1 << COMANDO_FECHAR_ESCRITA)
    | (1 << COMANDO_DEVOLVER_FICHAS)
    | (1 << COMANDO_MASCARA);
const MASCARA_DE_EVENTOS: i64 = (1 << EVENTO_ENTRADA)
    | (1 << EVENTO_SAIDA)
    | (1 << EVENTO_ERRO)
    | (1 << EVENTO_FECHAMENTO)
    | (1 << EVENTO_DESTRUIDO);

fn e_comando(dados: i64, bit: i64) -> bool {
    dados & MASCARA_DE_COMANDOS == 1 << bit
}

// ---------------------------------------------------------------------------
// O soquete nativo.

/// Um descritor que o Dart usa como `_NativeSocket` (o `Socket` da VM).
struct SoqueteNativo {
    descritor: std::sync::atomic::AtomicI64,
    /// A porta Dart dos eventos (a `eventPort` do `_NativeSocket`).
    porta: std::sync::atomic::AtomicI64,
}

impl SoqueteNativo {
    /// Cria o soquete com uma referência e devolve o ponteiro dela.
    fn novo(descritor: i64) -> i64 {
        std::sync::Arc::into_raw(std::sync::Arc::new(SoqueteNativo {
            descritor: std::sync::atomic::AtomicI64::new(descritor),
            porta: std::sync::atomic::AtomicI64::new(0),
        })) as i64
    }

    /// # Safety
    /// `p` veio de [`SoqueteNativo::novo`] e a referência ainda existe.
    unsafe fn de<'a>(p: i64) -> &'a SoqueteNativo {
        // SAFETY: garantido por quem chama.
        unsafe { &*(p as *const SoqueteNativo) }
    }

    fn reter(p: i64) {
        // SAFETY: `p` é um ponteiro vivo de `novo`.
        unsafe { std::sync::Arc::increment_strong_count(p as *const SoqueteNativo) };
    }

    fn liberar(p: i64) {
        // SAFETY: `p` é um ponteiro vivo de `novo`, com esta referência.
        unsafe { std::sync::Arc::decrement_strong_count(p as *const SoqueteNativo) };
    }

    fn descritor(&self) -> i64 {
        self.descritor.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Fecha o recurso do sistema (o descritor, no Unix; a referência do
    /// `Handle`, no Windows, cujo fechamento o manipulador já fez).
    fn fechar_descritor(&self) {
        let d = self.descritor.swap(-1, std::sync::atomic::Ordering::AcqRel);
        if d >= 0 {
            fechar_descritor_de_soquete(d);
        }
    }

    /// `Socket::CloseFd`: o objeto deixa o descritor sem fechá-lo no Unix
    /// (quem fecha é o manipulador, ou ninguém: a entrada padrão); no
    /// Windows solta a referência do `Handle`.
    fn soltar_descritor(&self) {
        #[cfg(unix)]
        self.descritor.store(-1, std::sync::atomic::Ordering::Release);
        #[cfg(windows)]
        self.fechar_descritor();
    }
}

/// No Windows cada objeto é dono de uma referência do `Handle` (o
/// `descritor`); ela é solta com o objeto, se ainda não foi.
#[cfg(windows)]
impl Drop for SoqueteNativo {
    fn drop(&mut self) {
        let d = *self.descritor.get_mut();
        if d >= 0 {
            fechar_descritor_de_soquete(d);
        }
    }
}

#[cfg(unix)]
fn fechar_descritor_de_soquete(d: i64) {
    unsafe extern "C" {
        fn close(fd: i32) -> i32;
    }
    // SAFETY: `d` é um descritor deste soquete, fechado uma vez.
    unsafe { close(d as i32) };
}

/// Outra referência ao descritor de um soquete de escuta compartilhado
/// (no Unix o descritor é o mesmo, sem contagem).
#[cfg(unix)]
fn reter_descritor(d: i64) -> i64 {
    d
}

/// O soquete da entrada padrão (`SocketBase::GetStdioHandle`): o próprio
/// descritor no Unix.
#[cfg(unix)]
fn manipulador_padrao(num: i64) -> i64 {
    descritor_padrao(num)
}

/// Os finalizadores do `_NativeSocket` coletado sem `close` (os
/// `*SocketFinalizer` de `socket.cc`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum FinalizadorDeSoquete {
    Normal,
    Escuta,
    Stdio,
    Sinal,
}

fn finalizar_soquete_normal(par: usize) {
    fechar_soquete_coletado(par as i64, 1 << COMANDO_FECHAR);
}

fn finalizar_soquete_de_escuta(par: usize) {
    fechar_soquete_coletado(par as i64, (1 << SOQUETE_DE_ESCUTA) | (1 << COMANDO_FECHAR));
}

fn finalizar_soquete_de_sinal(par: usize) {
    fechar_soquete_coletado(par as i64, (1 << SOQUETE_DE_SINAL) | (1 << COMANDO_FECHAR));
}

fn finalizar_soquete_stdio(par: usize) {
    // SAFETY: a referência do objeto ainda existe; é solta aqui.
    unsafe { SoqueteNativo::de(par as i64) }.soltar_descritor();
    SoqueteNativo::liberar(par as i64);
}

/// Envia o comando de fechar de um soquete coletado ao manipulador: a
/// referência do objeto passa ao comando, que a solta.
fn fechar_soquete_coletado(p: i64, dados: i64) {
    // SAFETY: a referência do objeto coletado ainda existe.
    let porta = unsafe { SoqueteNativo::de(p) }.porta.load(std::sync::atomic::Ordering::Acquire);
    manipulador_de_eventos().enviar(ComandoDeEvento { soquete: p, porta, dados });
}

/// `Socket::SetSocketIdNativeField`: cria o soquete de `descritor` no campo
/// nativo de `objeto`, com o finalizador.
fn definir_soquete_no_objeto(objeto: i64, descritor: i64, finalizador: FinalizadorDeSoquete) {
    reusar_soquete_no_objeto(objeto, SoqueteNativo::novo(descritor), finalizador);
}

/// `Socket::ReuseSocketIdNativeField`.
fn reusar_soquete_no_objeto(objeto: i64, p: i64, finalizador: FinalizadorDeSoquete) {
    gravar_campo_nativo(objeto, p);
    let f: fn(usize) = match finalizador {
        FinalizadorDeSoquete::Normal => finalizar_soquete_normal,
        FinalizadorDeSoquete::Escuta => finalizar_soquete_de_escuta,
        FinalizadorDeSoquete::Stdio => finalizar_soquete_stdio,
        FinalizadorDeSoquete::Sinal => finalizar_soquete_de_sinal,
    };
    anexar_finalizador(objeto, f, p as usize);
}

/// `Socket::GetSocketIdNativeField`: o soquete do objeto, ou lança o erro
/// interno ("No native peer").
fn soquete_do_objeto<'a>(objeto: i64) -> Option<&'a SoqueteNativo> {
    let p = campo_nativo(objeto);
    if p == 0 {
        lancar_erro_interno("No native peer");
        return None;
    }
    // SAFETY: a referência do objeto vive até a coleta dele.
    Some(unsafe { SoqueteNativo::de(p) })
}

// ---------------------------------------------------------------------------
// As fichas e os interessados de um descritor (`DescriptorInfo*Mixin`).

/// O que o manipulador sabe de um descritor.
enum InfoDeDescritor {
    /// Um soquete comum: uma porta, uma máscara e 16 fichas.
    Unico { porta: i64, mascara: i64, fichas: i64 },
    /// Um soquete de escuta compartilhado: cada porta com 4 fichas e se
    /// quer ler; as prontas se revezam (`active_readers_`).
    Multiplo { portas: Vec<EntradaDePorta>, ativos: std::collections::VecDeque<i64> },
}

struct EntradaDePorta {
    porta: i64,
    lendo: bool,
    fichas: i64,
}

impl EntradaDePorta {
    fn pronta(&self) -> bool {
        self.fichas > 0 && self.lendo
    }
}

const FICHAS_DE_UNICO: i64 = 16;
const FICHAS_DE_MULTIPLO: i64 = 4;

impl InfoDeDescritor {
    fn novo(de_escuta: bool) -> InfoDeDescritor {
        if de_escuta {
            InfoDeDescritor::Multiplo { portas: Vec::new(), ativos: std::collections::VecDeque::new() }
        } else {
            InfoDeDescritor::Unico { porta: 0, mascara: 0, fichas: FICHAS_DE_UNICO }
        }
    }

    fn de_escuta(&self) -> bool {
        matches!(self, InfoDeDescritor::Multiplo { .. })
    }

    fn definir_porta_e_mascara(&mut self, p: i64, m: i64) {
        match self {
            InfoDeDescritor::Unico { porta, mascara, .. } => {
                *porta = p;
                *mascara = m;
            }
            InfoDeDescritor::Multiplo { portas, ativos } => {
                let lendo = m == 1 << EVENTO_ENTRADA;
                match portas.iter_mut().find(|e| e.porta == p) {
                    None => {
                        let e = EntradaDePorta { porta: p, lendo, fichas: FICHAS_DE_MULTIPLO };
                        if e.pronta() {
                            ativos.push_back(p);
                        }
                        portas.push(e);
                    }
                    Some(e) => {
                        let estava = e.pronta();
                        e.lendo = lendo;
                        let esta = e.pronta();
                        if estava && !esta {
                            ativos.retain(|&x| x != p);
                        } else if !estava && esta {
                            ativos.push_back(p);
                        }
                    }
                }
            }
        }
    }

    fn remover_porta(&mut self, p: i64) {
        match self {
            InfoDeDescritor::Unico { porta, mascara, .. } => {
                *porta = 0;
                *mascara = 0;
            }
            InfoDeDescritor::Multiplo { portas, ativos } => {
                portas.retain(|e| e.porta != p);
                ativos.retain(|&x| x != p);
            }
        }
    }

    /// A porta a avisar de um evento de E/S (e uma ficha a menos).
    fn proxima_porta(&mut self) -> i64 {
        match self {
            InfoDeDescritor::Unico { porta, fichas, .. } => {
                *fichas -= 1;
                *porta
            }
            InfoDeDescritor::Multiplo { portas, ativos } => {
                let Some(p) = ativos.pop_front() else { return 0 };
                if let Some(e) = portas.iter_mut().find(|e| e.porta == p) {
                    e.fichas -= 1;
                    if e.pronta() {
                        ativos.push_back(p);
                    }
                }
                p
            }
        }
    }

    /// Avisa todas as portas (fechamento, erro, destruição).
    fn avisar_todas(&mut self, eventos: i64) {
        match self {
            InfoDeDescritor::Unico { porta, fichas, .. } => {
                if *porta != 0 {
                    postar_evento(*porta, eventos);
                }
                *fichas -= 1;
            }
            InfoDeDescritor::Multiplo { portas, ativos } => {
                for e in portas.iter_mut() {
                    postar_evento(e.porta, eventos);
                    let estava = e.pronta();
                    e.fichas -= 1;
                    if estava && !e.pronta() {
                        let p = e.porta;
                        ativos.retain(|&x| x != p);
                    }
                }
            }
        }
    }

    fn devolver_fichas(&mut self, p: i64, n: i64) {
        match self {
            InfoDeDescritor::Unico { fichas, .. } => *fichas += n,
            InfoDeDescritor::Multiplo { portas, ativos } => {
                if let Some(e) = portas.iter_mut().find(|e| e.porta == p) {
                    let estava = e.pronta();
                    e.fichas += n;
                    if !estava && e.pronta() {
                        ativos.push_back(p);
                    }
                }
            }
        }
    }

    /// A máscara a vigiar agora (vazia sem fichas).
    /// `RemoveAllPorts`.
    fn remover_todas(&mut self) {
        match self {
            InfoDeDescritor::Unico { porta, mascara, .. } => {
                *porta = 0;
                *mascara = 0;
            }
            InfoDeDescritor::Multiplo { portas, ativos } => {
                portas.clear();
                ativos.clear();
            }
        }
    }

    fn mascara(&self) -> i64 {
        match self {
            InfoDeDescritor::Unico { mascara, fichas, .. } => {
                if *fichas <= 0 { 0 } else { *mascara }
            }
            InfoDeDescritor::Multiplo { ativos, .. } => {
                if ativos.is_empty() { 0 } else { 1 << EVENTO_ENTRADA }
            }
        }
    }
}

/// `DartUtils::PostInt32`: a máscara de eventos na porta.
fn postar_evento(porta: i64, eventos: i64) {
    if porta != 0 {
        postar(porta, Grafo::escalar(eventos, ValueTag::Int));
    }
}

// ---------------------------------------------------------------------------
// O manipulador.

/// Um pedido ao manipulador (o `InterruptMessage` da VM): o soquete (com a
/// referência que o pedido carrega), a porta e os dados.
struct ComandoDeEvento {
    soquete: i64,
    porta: i64,
    dados: i64,
}

/// A thread do manipulador e a fila de pedidos dela; um byte no pipe de
/// interrupção a acorda. No Windows o pedido vai pela porta de conclusão
/// (`io_windows_eventos.rs`).
struct ManipuladorDeEventos {
    #[cfg(unix)]
    pedidos: std::sync::Mutex<Vec<ComandoDeEvento>>,
    #[cfg(unix)]
    interrupcao: i32,
    #[cfg(windows)]
    porta: usize,
}

fn manipulador_de_eventos() -> &'static ManipuladorDeEventos {
    static M: std::sync::OnceLock<ManipuladorDeEventos> = std::sync::OnceLock::new();
    M.get_or_init(iniciar_manipulador)
}

#[cfg(unix)]
impl ManipuladorDeEventos {
    fn enviar(&self, c: ComandoDeEvento) {
        self.pedidos.lock().unwrap_or_else(|e| e.into_inner()).push(c);
        self.acordar();
    }

    fn acordar(&self) {
        unsafe extern "C" {
            fn write(fd: i32, b: *const std::ffi::c_void, n: usize) -> isize;
        }
        let b = [1u8];
        // SAFETY: o pipe vive o processo inteiro; um byte basta para acordar
        // (o pipe cheio já acorda o manipulador, então a falha é inócua).
        unsafe { write(self.interrupcao, b.as_ptr().cast(), 1) };
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn pipe(fds: *mut i32) -> i32;
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn read(fd: i32, b: *mut std::ffi::c_void, n: usize) -> isize;
    fn shutdown(fd: i32, como: i32) -> i32;
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
fn tornar_nao_bloqueante(fd: i32) -> bool {
    // SAFETY: consulta e muda as bandeiras de um descritor aberto.
    unsafe {
        let f = fcntl(fd, F_GETFL);
        f >= 0 && fcntl(fd, F_SETFL, f | O_NONBLOCK) >= 0
    }
}

/// `FDUtils::SetCloseOnExec`.
#[cfg(unix)]
fn fechar_no_exec(fd: i32) -> bool {
    // SAFETY: muda as bandeiras de um descritor aberto.
    unsafe { fcntl(fd, F_SETFD, FD_CLOEXEC) >= 0 }
}

#[cfg(unix)]
fn iniciar_manipulador() -> ManipuladorDeEventos {
    let mut fds = [0i32; 2];
    // SAFETY: `fds` tem dois inteiros.
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        panic!("dart:io: falha ao criar o pipe do manipulador de eventos");
    }
    tornar_nao_bloqueante(fds[0]);
    tornar_nao_bloqueante(fds[1]);
    fechar_no_exec(fds[0]);
    fechar_no_exec(fds[1]);
    let leitura = fds[0];
    std::thread::Builder::new()
        .name("dart:io EventHandler".to_string())
        .spawn(move || LacoDeEventos::novo(leitura).rodar())
        .expect("dart:io: falha ao criar a thread do manipulador de eventos");
    ManipuladorDeEventos { pedidos: std::sync::Mutex::new(Vec::new()), interrupcao: fds[1] }
}

/// O estado da thread do manipulador.
#[cfg(unix)]
struct LacoDeEventos {
    sondagem: Sondagem,
    interrupcao: i32,
    descritores: std::collections::HashMap<i64, InfoDeDescritor>,
}

#[cfg(unix)]
impl LacoDeEventos {
    fn novo(interrupcao: i32) -> LacoDeEventos {
        let sondagem = Sondagem::nova(interrupcao);
        LacoDeEventos { sondagem, interrupcao, descritores: std::collections::HashMap::new() }
    }

    fn rodar(mut self) {
        let mut prontos = Vec::new();
        loop {
            prontos.clear();
            self.sondagem.esperar(&mut prontos);
            let mut interrompido = false;
            for &(fd, eventos) in &prontos {
                if fd < 0 {
                    interrompido = true;
                    continue;
                }
                let Some(di) = self.descritores.get_mut(&fd) else { continue };
                let antiga = di.mascara();
                if eventos & (1 << EVENTO_ERRO) != 0 {
                    di.avisar_todas(eventos);
                    self.atualizar(fd, antiga);
                } else if eventos != 0 {
                    let porta = di.proxima_porta();
                    self.atualizar(fd, antiga);
                    postar_evento(porta, eventos);
                }
            }
            if interrompido {
                self.tratar_interrupcao();
            }
        }
    }

    /// Reconcilia a sondagem com a máscara nova do descritor
    /// (`UpdateEpollInstance`).
    fn atualizar(&mut self, fd: i64, antiga: i64) {
        let Some(di) = self.descritores.get_mut(&fd) else { return };
        let nova = di.mascara();
        let de_escuta = di.de_escuta();
        if antiga != 0 && nova == 0 {
            self.sondagem.remover(fd);
        } else if antiga == 0 && nova != 0 {
            if !self.sondagem.adicionar(fd, nova, de_escuta) {
                di.avisar_todas(1 << EVENTO_FECHAMENTO);
            }
        } else if antiga != 0 && nova != 0 && antiga != nova {
            self.sondagem.remover(fd);
            if !self.sondagem.adicionar(fd, nova, de_escuta) {
                di.avisar_todas(1 << EVENTO_FECHAMENTO);
            }
        }
    }

    /// `HandleInterruptFd`: esvazia o pipe e atende os pedidos.
    fn tratar_interrupcao(&mut self) {
        let mut lixo = [0u8; 256];
        // SAFETY: o pipe é não bloqueante; lê até esvaziar.
        while unsafe { read(self.interrupcao, lixo.as_mut_ptr().cast(), lixo.len()) } > 0 {}
        let pedidos = std::mem::take(&mut *manipulador_de_eventos().pedidos.lock().unwrap_or_else(|e| e.into_inner()));
        for c in pedidos {
            self.tratar_comando(c);
        }
    }

    fn tratar_comando(&mut self, c: ComandoDeEvento) {
        // O pedido carrega uma referência do soquete (solta ao fim).
        // SAFETY: `c.soquete` veio de `SoqueteNativo::novo`, retido para o
        // pedido.
        let soquete = unsafe { std::sync::Arc::from_raw(c.soquete as *const SoqueteNativo) };
        let fd = soquete.descritor();
        if fd < 0 {
            return;
        }
        let de_escuta = c.dados & (1 << SOQUETE_DE_ESCUTA) != 0;
        let di = self.descritores.entry(fd).or_insert_with(|| InfoDeDescritor::novo(de_escuta));
        if e_comando(c.dados, COMANDO_FECHAR_LEITURA) {
            // SAFETY: descritor aberto deste soquete (SHUT_RD = 0).
            unsafe { shutdown(fd as i32, 0) };
        } else if e_comando(c.dados, COMANDO_FECHAR_ESCRITA) {
            // SAFETY: descritor aberto deste soquete (SHUT_WR = 1).
            unsafe { shutdown(fd as i32, 1) };
        } else if e_comando(c.dados, COMANDO_FECHAR) {
            if c.dados & (1 << SOQUETE_DE_SINAL) != 0 {
                limpar_sinal_por_descritor(fd);
            }
            let antiga = di.mascara();
            if c.porta != 0 {
                di.remover_porta(c.porta);
            }
            self.atualizar(fd, antiga);
            if de_escuta {
                // Outros objetos podem dividir o descritor: só o último
                // fecha (`CloseSafe`).
                if registro_de_escuta().fechar(c.soquete) {
                    self.descritores.remove(&fd);
                    soquete.fechar_descritor();
                } else {
                    soquete.soltar_descritor();
                }
            } else {
                self.descritores.remove(&fd);
                soquete.fechar_descritor();
            }
            postar_evento(c.porta, 1 << EVENTO_DESTRUIDO);
        } else if e_comando(c.dados, COMANDO_DEVOLVER_FICHAS) {
            let antiga = di.mascara();
            di.devolver_fichas(c.porta, c.dados & ((1 << COMANDO_FECHAR) - 1));
            self.atualizar(fd, antiga);
        } else if e_comando(c.dados, COMANDO_MASCARA) {
            let antiga = di.mascara();
            di.definir_porta_e_mascara(c.porta, c.dados & MASCARA_DE_EVENTOS);
            self.atualizar(fd, antiga);
        }
    }
}

// --- epoll (Linux) ---------------------------------------------------------

#[cfg(target_os = "linux")]
#[cfg_attr(target_arch = "x86_64", repr(C, packed))]
#[cfg_attr(not(target_arch = "x86_64"), repr(C))]
#[derive(Clone, Copy)]
struct EventoEpoll {
    eventos: u32,
    dados: u64,
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn epoll_create1(flags: i32) -> i32;
    fn epoll_ctl(ep: i32, op: i32, fd: i32, ev: *mut EventoEpoll) -> i32;
    fn epoll_wait(ep: i32, evs: *mut EventoEpoll, max: i32, espera: i32) -> i32;
}

#[cfg(target_os = "linux")]
struct Sondagem {
    epoll: i32,
}

#[cfg(target_os = "linux")]
impl Sondagem {
    const EPOLLIN: u32 = 0x1;
    const EPOLLOUT: u32 = 0x4;
    const EPOLLERR: u32 = 0x8;
    const EPOLLHUP: u32 = 0x10;
    const EPOLLRDHUP: u32 = 0x2000;
    const EPOLLET: u32 = 1 << 31;

    fn nova(interrupcao: i32) -> Sondagem {
        // SAFETY: EPOLL_CLOEXEC = O_CLOEXEC.
        let epoll = unsafe { epoll_create1(0o2000000) };
        if epoll < 0 {
            panic!("dart:io: falha ao criar o epoll");
        }
        // O pipe de interrupção entra com o dado 0.
        let mut ev = EventoEpoll { eventos: Self::EPOLLIN, dados: 0 };
        // SAFETY: `ev` é válido durante a chamada.
        if unsafe { epoll_ctl(epoll, 1, interrupcao, &mut ev) } != 0 {
            panic!("dart:io: falha ao vigiar o pipe de interrupção");
        }
        Sondagem { epoll }
    }

    /// `AddToEpollInstance`: `false` se o descritor não pôde entrar.
    fn adicionar(&self, fd: i64, mascara: i64, de_escuta: bool) -> bool {
        let mut eventos = Self::EPOLLRDHUP;
        if mascara & (1 << EVENTO_ENTRADA) != 0 {
            eventos |= Self::EPOLLIN;
        }
        if mascara & (1 << EVENTO_SAIDA) != 0 {
            eventos |= Self::EPOLLOUT;
        }
        if !de_escuta {
            eventos |= Self::EPOLLET;
        }
        let mut ev = EventoEpoll { eventos, dados: fd as u64 + 1 };
        // SAFETY: `ev` é válido durante a chamada (EPOLL_CTL_ADD = 1).
        unsafe { epoll_ctl(self.epoll, 1, fd as i32, &mut ev) == 0 }
    }

    fn remover(&self, fd: i64) {
        // SAFETY: EPOLL_CTL_DEL = 2; o evento é ignorado.
        unsafe { epoll_ctl(self.epoll, 2, fd as i32, std::ptr::null_mut()) };
    }

    /// Espera eventos: (descritor, máscara Dart); o descritor -1 é o pipe
    /// de interrupção.
    fn esperar(&self, saida: &mut Vec<(i64, i64)>) {
        let mut evs = [EventoEpoll { eventos: 0, dados: 0 }; 16];
        // SAFETY: o buffer tem 16 eventos.
        let n = unsafe { epoll_wait(self.epoll, evs.as_mut_ptr(), 16, -1) };
        for ev in evs.iter().take(n.max(0) as usize) {
            let (eventos, dados) = (ev.eventos, ev.dados);
            if dados == 0 {
                saida.push((-1, 0));
                continue;
            }
            saida.push((dados as i64 - 1, Self::mascara_dart(eventos)));
        }
    }

    /// `GetPollEvents`.
    fn mascara_dart(eventos: u32) -> i64 {
        if eventos & Self::EPOLLERR != 0 {
            return if eventos & Self::EPOLLIN != 0 { 1 << EVENTO_ERRO } else { 0 };
        }
        let mut m = 0;
        if eventos & Self::EPOLLIN != 0 {
            m |= 1 << EVENTO_ENTRADA;
        }
        if eventos & Self::EPOLLOUT != 0 {
            m |= 1 << EVENTO_SAIDA;
        }
        if eventos & (Self::EPOLLHUP | Self::EPOLLRDHUP) != 0 {
            m |= 1 << EVENTO_FECHAMENTO;
        }
        m
    }
}

// --- kqueue (macOS e BSDs) -------------------------------------------------

#[cfg(all(unix, not(target_os = "linux")))]
#[repr(C)]
#[derive(Clone, Copy)]
struct EventoKqueue {
    ident: usize,
    filtro: i16,
    bandeiras: u16,
    fbandeiras: u32,
    dados: isize,
    udata: *mut std::ffi::c_void,
}

#[cfg(all(unix, not(target_os = "linux")))]
unsafe extern "C" {
    fn kqueue() -> i32;
    fn kevent(kq: i32, mudancas: *const EventoKqueue, nm: i32, eventos: *mut EventoKqueue, ne: i32, espera: *const std::ffi::c_void) -> i32;
}

#[cfg(all(unix, not(target_os = "linux")))]
struct Sondagem {
    kq: i32,
    /// Os descritores que estão no kqueue e se são de escuta.
    vigiados: std::cell::RefCell<std::collections::HashMap<i64, bool>>,
}

#[cfg(all(unix, not(target_os = "linux")))]
impl Sondagem {
    const EVFILT_READ: i16 = -1;
    const EVFILT_WRITE: i16 = -2;
    const EV_ADD: u16 = 0x1;
    const EV_DELETE: u16 = 0x2;
    const EV_CLEAR: u16 = 0x20;
    const EV_EOF: u16 = 0x8000;
    const EV_ERROR: u16 = 0x4000;

    fn evento(fd: i64, filtro: i16, bandeiras: u16) -> EventoKqueue {
        EventoKqueue { ident: fd as usize, filtro, bandeiras, fbandeiras: 0, dados: 0, udata: (fd + 1) as usize as *mut std::ffi::c_void }
    }

    fn nova(interrupcao: i32) -> Sondagem {
        // SAFETY: cria o kqueue; o evento vive durante a chamada.
        let kq = unsafe { kqueue() };
        if kq < 0 {
            panic!("dart:io: falha ao criar o kqueue");
        }
        fechar_no_exec(kq);
        let mut ev = Self::evento(i64::from(interrupcao), Self::EVFILT_READ, Self::EV_ADD);
        ev.udata = std::ptr::null_mut();
        if unsafe { kevent(kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) } == -1 {
            panic!("dart:io: falha ao vigiar o pipe de interrupção");
        }
        Sondagem { kq, vigiados: std::cell::RefCell::new(std::collections::HashMap::new()) }
    }

    /// `AddToKqueue`.
    fn adicionar(&self, fd: i64, mascara: i64, de_escuta: bool) -> bool {
        let mut bandeiras = Self::EV_ADD;
        if !de_escuta {
            bandeiras |= Self::EV_CLEAR;
        }
        let mut mudancas = Vec::with_capacity(2);
        if mascara & (1 << EVENTO_ENTRADA) != 0 {
            mudancas.push(Self::evento(fd, Self::EVFILT_READ, bandeiras));
        }
        if mascara & (1 << EVENTO_SAIDA) != 0 {
            mudancas.push(Self::evento(fd, Self::EVFILT_WRITE, bandeiras));
        }
        // SAFETY: as mudanças vivem durante a chamada.
        let ok = unsafe { kevent(self.kq, mudancas.as_ptr(), mudancas.len() as i32, std::ptr::null_mut(), 0, std::ptr::null()) } != -1;
        if ok {
            self.vigiados.borrow_mut().insert(fd, de_escuta);
        }
        ok
    }

    /// `RemoveFromKqueue`.
    fn remover(&self, fd: i64) {
        if self.vigiados.borrow_mut().remove(&fd).is_none() {
            return;
        }
        for filtro in [Self::EVFILT_READ, Self::EVFILT_WRITE] {
            let ev = Self::evento(fd, filtro, Self::EV_DELETE);
            // SAFETY: a mudança vive durante a chamada; a ausência do filtro
            // é inócua.
            unsafe { kevent(self.kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
        }
    }

    fn esperar(&self, saida: &mut Vec<(i64, i64)>) {
        let vazio = Self::evento(0, 0, 0);
        let mut evs = [vazio; 16];
        // SAFETY: o buffer tem 16 eventos; espera sem prazo.
        let n = unsafe { kevent(self.kq, std::ptr::null(), 0, evs.as_mut_ptr(), 16, std::ptr::null()) };
        for ev in evs.iter().take(n.max(0) as usize) {
            if ev.bandeiras & Self::EV_ERROR != 0 {
                continue;
            }
            if ev.udata.is_null() {
                saida.push((-1, 0));
                continue;
            }
            let fd = ev.udata as usize as i64 - 1;
            let de_escuta = self.vigiados.borrow().get(&fd).copied().unwrap_or(false);
            saida.push((fd, Self::mascara_dart(ev, de_escuta)));
        }
    }

    /// `GetEvents` de `eventhandler_macos.cc`.
    fn mascara_dart(ev: &EventoKqueue, de_escuta: bool) -> i64 {
        let eof = ev.bandeiras & Self::EV_EOF != 0;
        if de_escuta {
            let mut m = 0;
            if ev.filtro == Self::EVFILT_READ && eof {
                m = if ev.fbandeiras != 0 { 1 << EVENTO_ERRO } else { 1 << EVENTO_FECHAMENTO };
            }
            if m == 0 {
                m = 1 << EVENTO_ENTRADA;
            }
            return m;
        }
        if ev.filtro == Self::EVFILT_READ {
            let mut m = 1 << EVENTO_ENTRADA;
            if eof {
                if ev.fbandeiras != 0 {
                    m = 1 << EVENTO_ERRO;
                } else {
                    m |= 1 << EVENTO_FECHAMENTO;
                }
            }
            m
        } else {
            let mut m = 1 << EVENTO_SAIDA;
            if eof && ev.fbandeiras != 0 {
                m = 1 << EVENTO_ERRO;
            }
            m
        }
    }
}

// ---------------------------------------------------------------------------
// O registro dos soquetes de escuta (`ListeningSocketRegistry`): o mesmo
// (endereço, porta) aberto por várias `ServerSocket` com `shared: true`
// divide um descritor.

/// Um soquete de escuta do sistema e quantos objetos Dart o usam.
struct SoqueteDeEscuta {
    endereco: Vec<u8>,
    porta: i64,
    so_v6: bool,
    compartilhado: bool,
    usos: usize,
    descritor: i64,
    /// O caminho de um soquete de domínio Unix (apagado no último uso).
    caminho_unix: Option<Vec<u8>>,
}

#[derive(Default)]
struct RegistroDeEscuta {
    /// Os soquetes do sistema.
    soquetes: Vec<SoqueteDeEscuta>,
    /// Soquete nativo (ponteiro) → descritor do sistema.
    por_soquete: std::collections::HashMap<i64, i64>,
}

fn registro_de_escuta() -> std::sync::MutexGuard<'static, RegistroDeEscuta> {
    static R: std::sync::OnceLock<std::sync::Mutex<RegistroDeEscuta>> = std::sync::OnceLock::new();
    R.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

impl RegistroDeEscuta {
    /// `CloseSafe`: solta um uso; `true` se o descritor do sistema deve ser
    /// fechado (era o último).
    fn fechar(&mut self, soquete: i64) -> bool {
        let Some(fd) = self.por_soquete.remove(&soquete) else {
            return true;
        };
        let Some(i) = self.soquetes.iter().position(|s| s.descritor == fd) else {
            return true;
        };
        self.soquetes[i].usos -= 1;
        if self.soquetes[i].usos > 0 {
            return false;
        }
        let s = self.soquetes.remove(i);
        #[cfg(unix)]
        if let Some(c) = s.caminho_unix {
            apagar_caminho_unix(&c);
        }
        #[cfg(windows)]
        let _ = s;
        true
    }
}

// ---------------------------------------------------------------------------
// Os natives do `_EventHandler`.

/// `EventHandler_SendData(sender, sendPort, data)`: o comando `data` para o
/// soquete de `sender` (que passa a avisar em `sendPort`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_EventHandler_SendData(remetente: i64, porta: i64, dados: i64) {
    let porta = campo_nativo(porta);
    if remetente == 0 {
        // Os timers da VM passam por aqui; os do nativo, não
        // (`eventos.rs`).
        return;
    }
    let p = campo_nativo(remetente);
    if p == 0 {
        lancar_erro_interno("No native peer");
        return;
    }
    // SAFETY: a referência do objeto está viva.
    unsafe { SoqueteNativo::de(p) }.porta.store(porta, std::sync::atomic::Ordering::Release);
    SoqueteNativo::reter(p);
    manipulador_de_eventos().enviar(ComandoDeEvento { soquete: p, porta, dados });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_EventHandler_TimerMillisecondClock() -> i64 {
    dartforge_nativo_Stopwatch_now() / 1_000_000
}
