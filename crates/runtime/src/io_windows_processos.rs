// Runtime nativo: os processos do `dart:io` no Windows (`process_win.cc`
// da VM).
//
// O filho nasce de `CreateProcessW` com a linha de comando que o Dart já
// escapou (`_windowsArgumentEscape`), o ambiente em UTF-16 e, fora do modo
// `inheritStdio`, só os três handles de E/S herdados
// (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`: nenhum outro handle do processo
// vaza para o filho). A E/S padrão do filho são pipes nomeados: o lado do
// pai é sobreposto e vira um [`Manipulador`] na porta de conclusão; o do
// filho é síncrono, como um programa espera. O código de saída chega pelo
// pool de threads do sistema (`RegisterWaitForSingleObject`), que o escreve
// (`[código, negativo]`, dois `int32`) no pipe de saída do processo.
//
// Os sinais são os eventos de console (`SetConsoleCtrlHandler`): SIGINT é
// o Ctrl+C e SIGHUP o fechamento da janela.

#[cfg(windows)]
mod processos_windows {
    use super::*;
    use std::ffi::c_void;
    use std::sync::Arc;

    const PIPE_ACCESS_INBOUND: u32 = 0x1;
    const PIPE_ACCESS_OUTBOUND: u32 = 0x2;
    const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
    const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
    const PIPE_REJECT_REMOTE_CLIENTS: u32 = 0x8;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const FILE_WRITE_ATTRIBUTES: u32 = 0x100;
    const OPEN_EXISTING: u32 = 3;
    /// O buffer de cada sentido dos pipes (o do `libuv`).
    const TAMANHO_DO_PIPE: u32 = 64 * 1024;
    const STARTF_USESTDHANDLES: u32 = 0x100;
    const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
    const CREATE_UNICODE_ENVIRONMENT: u32 = 0x400;
    const DETACHED_PROCESS: u32 = 0x8;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
    const WT_EXECUTEONLYONCE: u32 = 0x8;
    const INFINITE: u32 = u32::MAX;
    const PROCESS_TERMINATE: u32 = 0x1;
    const WAIT_OBJECT_0: u32 = 0;
    const ERROR_NOT_SUPPORTED: i32 = 50;
    const CTRL_C_EVENT: u32 = 0;
    const CTRL_CLOSE_EVENT: u32 = 2;

    #[repr(C)]
    struct AtributosDeSeguranca {
        tamanho: u32,
        descritor: *mut c_void,
        herdar: i32,
    }

    /// `STARTUPINFOW`.
    #[repr(C)]
    struct InfoDeInicio {
        cb: u32,
        reservado: *mut u16,
        area_de_trabalho: *mut u16,
        titulo: *mut u16,
        x: u32,
        y: u32,
        largura: u32,
        altura: u32,
        colunas: u32,
        linhas: u32,
        atributo: u32,
        bandeiras: u32,
        mostrar: u16,
        reservado2_tamanho: u16,
        reservado2: *mut u8,
        entrada: Bruto,
        saida: Bruto,
        erro: Bruto,
    }

    /// `STARTUPINFOEXW`.
    #[repr(C)]
    struct InfoDeInicioEx {
        base: InfoDeInicio,
        atributos: *mut c_void,
    }

    /// `PROCESS_INFORMATION`.
    #[repr(C)]
    struct InfoDoProcesso {
        processo: Bruto,
        thread: Bruto,
        pid: u32,
        tid: u32,
    }

    type RetornoDeEspera = unsafe extern "system" fn(*mut c_void, u8);
    type TratadorDeConsole = unsafe extern "system" fn(u32) -> i32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateNamedPipeW(nome: *const u16, abertura: u32, modo: u32, instancias: u32, saida: u32, entrada: u32, espera: u32, seg: *const AtributosDeSeguranca) -> Bruto;
        fn CreateFileW(nome: *const u16, acesso: u32, compartilhar: u32, seg: *const AtributosDeSeguranca, disposicao: u32, bandeiras: u32, modelo: Bruto) -> Bruto;
        fn InitializeProcThreadAttributeList(lista: *mut c_void, n: u32, bandeiras: u32, tamanho: *mut usize) -> i32;
        fn UpdateProcThreadAttribute(lista: *mut c_void, bandeiras: u32, atributo: usize, valor: *const c_void, tamanho: usize, anterior: *mut c_void, retorno: *mut usize) -> i32;
        fn DeleteProcThreadAttributeList(lista: *mut c_void);
        fn CreateProcessW(
            aplicativo: *const u16,
            linha: *mut u16,
            atributos_do_processo: *const c_void,
            atributos_da_thread: *const c_void,
            herdar: i32,
            bandeiras: u32,
            ambiente: *const c_void,
            diretorio: *const u16,
            inicio: *const InfoDeInicio,
            info: *mut InfoDoProcesso,
        ) -> i32;
        fn RegisterWaitForSingleObject(espera: *mut Bruto, objeto: Bruto, retorno: RetornoDeEspera, contexto: *mut c_void, ms: u32, bandeiras: u32) -> i32;
        fn UnregisterWait(espera: Bruto) -> i32;
        fn GetExitCodeProcess(processo: Bruto, codigo: *mut u32) -> i32;
        fn TerminateProcess(processo: Bruto, codigo: u32) -> i32;
        fn OpenProcess(acesso: u32, herdar: i32, pid: u32) -> Bruto;
        fn CreateEventW(seg: *const c_void, manual: i32, inicial: i32, nome: *const u16) -> Bruto;
        fn WaitForMultipleObjects(n: u32, handles: *const Bruto, todos: i32, ms: u32) -> u32;
        fn GetOverlappedResult(h: Bruto, ov: *mut Sobreposto, n: *mut u32, esperar: i32) -> i32;
        fn SetConsoleCtrlHandler(tratador: Option<TratadorDeConsole>, adicionar: i32) -> i32;
        fn GetCurrentProcessId() -> u32;
    }

    fn erro_do_sistema() -> ErroDoSo {
        // SAFETY: lê o erro da thread.
        ErroDoSo::do_codigo(unsafe { GetLastError() } as i32)
    }

    fn largo(b: &[u8]) -> Vec<u16> {
        String::from_utf8_lossy(b).encode_utf16().collect()
    }

    /// Os handles de um início, fechados se ele falha (ou se não forem
    /// entregues a ninguém).
    #[derive(Default)]
    struct Guardados(Vec<Bruto>);

    impl Guardados {
        fn guardar(&mut self, hs: &[Bruto]) {
            self.0.extend(hs.iter().copied().filter(|&h| h != INVALIDO));
        }

        /// O handle passa a quem chama.
        fn entregar(&mut self, h: Bruto) -> Bruto {
            self.0.retain(|&x| x != h);
            h
        }

        fn fechar(&mut self, h: Bruto) {
            if h != INVALIDO {
                self.entregar(h);
                // SAFETY: handle deste início, fechado uma vez.
                unsafe { CloseHandle(h) };
            }
        }
    }

    impl Drop for Guardados {
        fn drop(&mut self) {
            for &h in &self.0 {
                // SAFETY: handles deste início que ninguém recebeu.
                unsafe { CloseHandle(h) };
            }
        }
    }

    /// Quem usa cada lado de um pipe.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum UsoDoPipe {
        /// A entrada do filho: o pai escreve (sobreposto), o filho lê.
        EntradaDoFilho,
        /// Uma saída do filho: o filho escreve, o pai lê (sobreposto).
        SaidaDoFilho,
        /// O pipe do código de saída: o pai lê (sobreposto); a escrita é
        /// síncrona e não vai ao filho.
        CodigoDeSaida,
        /// O pipe de um sinal: os dois lados sobrepostos, nenhum herdado.
        Sinal,
    }

    /// Um nome de pipe único no sistema (`\\.\pipe\dart-<pid>-<n>`); o
    /// `FILE_FLAG_FIRST_PIPE_INSTANCE` garante que ninguém o tomou antes.
    fn nome_de_pipe() -> Vec<u16> {
        static CONTADOR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = CONTADOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // SAFETY: consulta sem efeitos.
        let pid = unsafe { GetCurrentProcessId() };
        format!(r"\\.\pipe\dart-{pid}-{n}").encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// `CreateProcessPipe`: `[leitura, escrita]`.
    fn criar_pipe(uso: UsoDoPipe) -> ResultadoIo<[Bruto; 2]> {
        let nome = nome_de_pipe();
        let herdavel = AtributosDeSeguranca { tamanho: std::mem::size_of::<AtributosDeSeguranca>() as u32, descritor: std::ptr::null_mut(), herdar: 1 };
        let pai_escreve = uso == UsoDoPipe::EntradaDoFilho;
        let abertura = if pai_escreve { PIPE_ACCESS_OUTBOUND } else { PIPE_ACCESS_INBOUND } | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE;
        // SAFETY: cria o servidor do pipe com o nome único.
        let servidor = unsafe { CreateNamedPipeW(nome.as_ptr(), abertura, PIPE_REJECT_REMOTE_CLIENTS, 1, TAMANHO_DO_PIPE, TAMANHO_DO_PIPE, 0, std::ptr::null()) };
        if servidor == INVALIDO {
            return Err(erro_do_sistema());
        }
        let (acesso, seg, bandeiras) = match uso {
            UsoDoPipe::EntradaDoFilho => (GENERIC_READ | FILE_WRITE_ATTRIBUTES, &herdavel as *const _, 0),
            UsoDoPipe::SaidaDoFilho => (GENERIC_WRITE | FILE_READ_ATTRIBUTES, &herdavel as *const _, 0),
            UsoDoPipe::CodigoDeSaida => (GENERIC_WRITE, std::ptr::null(), 0),
            UsoDoPipe::Sinal => (GENERIC_WRITE, std::ptr::null(), FILE_FLAG_OVERLAPPED),
        };
        // SAFETY: abre o cliente do pipe recém-criado.
        let cliente = unsafe { CreateFileW(nome.as_ptr(), acesso, 0, seg, OPEN_EXISTING, bandeiras, 0) };
        if cliente == INVALIDO {
            let e = erro_do_sistema();
            // SAFETY: o servidor não foi entregue a ninguém.
            unsafe { CloseHandle(servidor) };
            return Err(e);
        }
        Ok(if pai_escreve { [cliente, servidor] } else { [servidor, cliente] })
    }

    /// `OpenNul`: o `NUL` herdável (a E/S dos processos destacados).
    fn abrir_nul() -> ResultadoIo<Bruto> {
        let herdavel = AtributosDeSeguranca { tamanho: std::mem::size_of::<AtributosDeSeguranca>() as u32, descritor: std::ptr::null_mut(), herdar: 1 };
        let nome: Vec<u16> = "NUL".encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: abre o dispositivo nulo.
        let h = unsafe { CreateFileW(nome.as_ptr(), GENERIC_READ | GENERIC_WRITE, 0, &herdavel, OPEN_EXISTING, 0, 0) };
        if h == INVALIDO { Err(erro_do_sistema()) } else { Ok(h) }
    }

    /// A lista de atributos com os handles herdados, apagada no fim.
    struct ListaDeAtributos(Vec<u64>);

    impl ListaDeAtributos {
        fn com_handles(handles: &[Bruto; 3]) -> ResultadoIo<ListaDeAtributos> {
            let mut tamanho = 0usize;
            // SAFETY: consulta o tamanho (falha com
            // ERROR_INSUFFICIENT_BUFFER, esperado).
            unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut tamanho) };
            let mut l = ListaDeAtributos(vec![0; tamanho.div_ceil(8).max(1)]);
            // SAFETY: o buffer tem `tamanho` bytes, alinhado a 8.
            if unsafe { InitializeProcThreadAttributeList(l.ptr(), 1, 0, &mut tamanho) } == 0 {
                let e = erro_do_sistema();
                std::mem::forget(l);
                return Err(e);
            }
            // SAFETY: `handles` vive até o fim do `CreateProcessW` (é de
            // quem chama, que mantém a lista por menos tempo).
            if unsafe {
                UpdateProcThreadAttribute(l.ptr(), 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, handles.as_ptr().cast(), std::mem::size_of_val(handles), std::ptr::null_mut(), std::ptr::null_mut())
            } == 0
            {
                return Err(erro_do_sistema());
            }
            Ok(l)
        }

        fn ptr(&mut self) -> *mut c_void {
            self.0.as_mut_ptr().cast()
        }
    }

    impl Drop for ListaDeAtributos {
        fn drop(&mut self) {
            // SAFETY: a lista foi iniciada.
            unsafe { DeleteProcThreadAttributeList(self.ptr()) };
        }
    }

    /// `ProcessStarter::Start`.
    pub(super) fn iniciar_processo(
        caminho: &[u8],
        argumentos: &[Vec<u8>],
        diretorio: Option<&[u8]>,
        ambiente: Option<&[Vec<u8>]>,
        modo: i64,
    ) -> Result<ProcessoIniciado, ErroDoSo> {
        // A linha de comando: o caminho e os argumentos, já escapados pelo
        // Dart, separados por espaço.
        let mut linha = largo(caminho);
        for a in argumentos {
            linha.push(u16::from(b' '));
            linha.extend(largo(a));
        }
        linha.push(0);
        // O bloco de ambiente: `NOME=valor\0…\0` (vazio: dois NULs).
        let bloco = ambiente.map(|v| {
            let mut b = Vec::new();
            for x in v {
                b.extend(largo(x));
                b.push(0);
            }
            if v.is_empty() {
                b.push(0);
            }
            b.push(0);
            b
        });
        let dir = diretorio.map(|d| {
            let mut w = largo(d);
            w.push(0);
            w
        });

        let mut g = Guardados::default();
        let mut entrada = [INVALIDO; 2];
        let mut saida = [INVALIDO; 2];
        let mut erro = [INVALIDO; 2];
        let mut fim = [INVALIDO; 2];
        if modo != MODO_DESTACADO {
            if modo_com_stdio(modo) {
                entrada = criar_pipe(UsoDoPipe::EntradaDoFilho)?;
                g.guardar(&entrada);
                saida = criar_pipe(UsoDoPipe::SaidaDoFilho)?;
                g.guardar(&saida);
                erro = criar_pipe(UsoDoPipe::SaidaDoFilho)?;
                g.guardar(&erro);
            }
            if modo_anexado(modo) {
                fim = criar_pipe(UsoDoPipe::CodigoDeSaida)?;
                g.guardar(&fim);
            }
        } else {
            entrada[0] = abrir_nul()?;
            g.guardar(&entrada);
            saida[1] = abrir_nul()?;
            g.guardar(&saida);
            erro[1] = abrir_nul()?;
            g.guardar(&erro);
        }

        // SAFETY: `InfoDeInicioEx` é POD; zero é o valor inicial do C.
        let mut inicio: InfoDeInicioEx = unsafe { std::mem::zeroed() };
        inicio.base.cb = std::mem::size_of::<InfoDeInicioEx>() as u32;
        let herdados = [entrada[0], saida[1], erro[1]];
        let mut lista = None;
        if modo != MODO_HERDAR_STDIO {
            inicio.base.entrada = entrada[0];
            inicio.base.saida = saida[1];
            inicio.base.erro = erro[1];
            inicio.base.bandeiras = STARTF_USESTDHANDLES;
            let l = lista.insert(ListaDeAtributos::com_handles(&herdados)?);
            inicio.atributos = l.ptr();
        }
        let mut bandeiras = EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT;
        if !modo_anexado(modo) {
            bandeiras |= DETACHED_PROCESS;
        } else if modo != MODO_HERDAR_STDIO {
            bandeiras |= CREATE_NO_WINDOW;
        }
        // SAFETY: `InfoDoProcesso` é POD.
        let mut info: InfoDoProcesso = unsafe { std::mem::zeroed() };
        // SAFETY: a linha é gravável e terminada em NUL; o ambiente, o
        // diretório, a lista de atributos e os handles vivem durante a
        // chamada.
        let ok = unsafe {
            CreateProcessW(
                std::ptr::null(),
                linha.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                bandeiras,
                bloco.as_ref().map_or(std::ptr::null(), |b| b.as_ptr().cast()),
                dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()),
                &inicio.base,
                &mut info,
            )
        };
        if ok == 0 {
            return Err(erro_do_sistema());
        }
        drop(lista);
        // Os lados do filho (ou o `NUL`) ficam só com ele.
        if modo != MODO_HERDAR_STDIO {
            for h in herdados {
                g.fechar(h);
            }
        }
        // SAFETY: a thread principal do filho não é usada.
        unsafe { CloseHandle(info.thread) };
        if modo_anexado(modo) {
            registrar_processo(info.pid, info.processo, g.entregar(fim[1]));
        } else {
            // SAFETY: o processo destacado não é acompanhado.
            unsafe { CloseHandle(info.processo) };
        }
        let mut r = ProcessoIniciado { pid: i64::from(info.pid), entrada: -1, saida: -1, erro: -1, fim: -1 };
        if modo != MODO_DESTACADO {
            let arquivo = |g: &mut Guardados, h: Bruto| descritor_de(Manipulador::novo(TipoDeManipulador::Arquivo, g.entregar(h)));
            if modo_com_stdio(modo) {
                r.entrada = arquivo(&mut g, entrada[1]);
                r.saida = arquivo(&mut g, saida[0]);
                r.erro = arquivo(&mut g, erro[0]);
            }
            if modo_anexado(modo) {
                r.fim = arquivo(&mut g, fim[0]);
            }
        }
        Ok(r)
    }

    // -----------------------------------------------------------------------
    // Os processos acompanhados (`ProcessInfoList`).

    struct ProcessoVivo {
        processo: Bruto,
        espera: Bruto,
        /// O lado de escrita do pipe do código de saída.
        pipe: Bruto,
    }

    fn processos_vivos() -> std::sync::MutexGuard<'static, crate::hash::HashMap<u32, ProcessoVivo>> {
        static P: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<u32, ProcessoVivo>>> = std::sync::OnceLock::new();
        P.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `ProcessInfoList::AddProcess`: o pool do sistema chama
    /// [`processo_terminou`] quando o filho sai. A trava cobre o registro,
    /// para o retorno achar o processo mesmo se ele já saiu.
    fn registrar_processo(pid: u32, processo: Bruto, pipe: Bruto) {
        let mut vivos = processos_vivos();
        let mut espera = 0;
        // SAFETY: espera o handle do processo, uma vez, no pool do sistema.
        if unsafe { RegisterWaitForSingleObject(&mut espera, processo, processo_terminou, pid as usize as *mut c_void, INFINITE, WT_EXECUTEONLYONCE) } == 0 {
            panic!("dart:io: falha ao registrar a espera do processo ({})", unsafe { GetLastError() });
        }
        vivos.insert(pid, ProcessoVivo { processo, espera, pipe });
    }

    /// `ExitCodeCallback`: escreve `[código, negativo]` no pipe de saída.
    unsafe extern "system" fn processo_terminou(contexto: *mut c_void, expirou: u8) {
        if expirou != 0 {
            return;
        }
        let pid = contexto as usize as u32;
        let Some(p) = processos_vivos().remove(&pid) else { return };
        // SAFETY: a espera é deste processo; dentro do retorno ela só pode
        // ser desfeita sem bloquear.
        unsafe { UnregisterWait(p.espera) };
        let mut codigo = 0u32;
        // SAFETY: o processo terminou; o handle é deste registro.
        unsafe { GetExitCodeProcess(p.processo, &mut codigo) };
        let c = codigo as i32;
        let (valor, negativo) = if c < 0 { (c.wrapping_abs(), 1i32) } else { (c, 0) };
        let mut msg = [0u8; 8];
        msg[..4].copy_from_slice(&valor.to_ne_bytes());
        msg[4..].copy_from_slice(&negativo.to_ne_bytes());
        let mut escritos = 0u32;
        // SAFETY: escrita síncrona no pipe do código de saída (falha só se
        // o Dart já fechou o outro lado, e aí ninguém espera o código).
        unsafe {
            WriteFile(p.pipe, msg.as_ptr(), 8, &mut escritos, std::ptr::null_mut());
            CloseHandle(p.pipe);
            CloseHandle(p.processo);
        }
    }

    /// `Process::Kill`: `TerminateProcess` com o código -1 (o sinal não se
    /// aplica).
    pub(super) fn matar_processo(pid: i64, _sinal: i64) -> bool {
        let Ok(pid) = u32::try_from(pid) else { return false };
        {
            let vivos = processos_vivos();
            if let Some(p) = vivos.get(&pid) {
                // SAFETY: o handle do processo acompanhado.
                return unsafe { TerminateProcess(p.processo, u32::MAX) } != 0;
            }
        }
        // SAFETY: abre um processo qualquer só para terminá-lo.
        let h = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
        if h == 0 {
            return false;
        }
        // SAFETY: o handle aberto acima, fechado uma vez.
        unsafe {
            let ok = TerminateProcess(h, u32::MAX) != 0;
            CloseHandle(h);
            ok
        }
    }

    // -----------------------------------------------------------------------
    // `Process::Wait` (o `runSync`).

    /// Uma leitura sobreposta com evento (sem passar pela porta de
    /// conclusão: o bit 0 do evento a desvia) até o fim do pipe.
    struct LeituraDeEspera {
        handle: Bruto,
        evento: Bruto,
        sobreposto: Box<Sobreposto>,
        buffer: Vec<u8>,
        dados: Vec<u8>,
        pendente: bool,
    }

    impl LeituraDeEspera {
        /// O leitor passa a ser dono de `handle` (fechado também se a
        /// criação falha).
        fn nova(handle: Bruto) -> ResultadoIo<LeituraDeEspera> {
            // SAFETY: um evento automático, sinalizado para a primeira
            // leitura começar.
            let evento = unsafe { CreateEventW(std::ptr::null(), 0, 1, std::ptr::null()) };
            if evento == 0 {
                let e = erro_do_sistema();
                // SAFETY: o handle é deste leitor.
                unsafe { CloseHandle(handle) };
                return Err(e);
            }
            Ok(LeituraDeEspera { handle, evento, sobreposto: Box::default(), buffer: vec![0; 16 * 1024], dados: Vec::new(), pendente: false })
        }

        /// Recolhe o que concluiu e emite leituras até uma ficar pendente;
        /// `Ok(false)` no fim do pipe.
        fn avancar(&mut self) -> ResultadoIo<bool> {
            loop {
                if self.pendente {
                    let mut n = 0u32;
                    // SAFETY: a leitura emitida com este `OVERLAPPED`.
                    if unsafe { GetOverlappedResult(self.handle, &mut *self.sobreposto, &mut n, 0) } == 0 {
                        return match unsafe { GetLastError() } {
                            ERROR_IO_INCOMPLETE => Ok(true),
                            ERROR_BROKEN_PIPE => Ok(false),
                            c => Err(ErroDoSo::do_codigo(c as i32)),
                        };
                    }
                    self.pendente = false;
                    self.dados.extend_from_slice(&self.buffer[..n as usize]);
                }
                *self.sobreposto = Sobreposto::com_evento(self.evento | 1);
                // SAFETY: o buffer e o `OVERLAPPED` vivem até a conclusão
                // (a leitura é recolhida ou o handle fechado antes de
                // soltá-los).
                let ok = unsafe { ReadFile(self.handle, self.buffer.as_mut_ptr(), self.buffer.len() as u32, std::ptr::null_mut(), &mut *self.sobreposto) } != 0;
                self.pendente = true;
                if !ok {
                    match unsafe { GetLastError() } {
                        ERROR_IO_PENDING => return Ok(true),
                        ERROR_BROKEN_PIPE => {
                            self.pendente = false;
                            return Ok(false);
                        }
                        c => {
                            self.pendente = false;
                            return Err(ErroDoSo::do_codigo(c as i32));
                        }
                    }
                }
            }
        }
    }

    impl Drop for LeituraDeEspera {
        fn drop(&mut self) {
            // SAFETY: fechar o handle aborta a leitura pendente; o evento é
            // deste leitor.
            unsafe {
                CloseHandle(self.handle);
                CloseHandle(self.evento);
            }
        }
    }

    /// Toma o handle de um manipulador de pipe: o manipulador não o fecha
    /// mais.
    fn tomar_handle(fd: i64) -> ResultadoIo<Bruto> {
        let m = manipulador_de(fd).ok_or_else(handle_invalido)?;
        m.estado().tomar_handle();
        Ok(m.bruto)
    }

    /// `Process::Wait`: fecha a entrada, lê a saída e o erro até o fim e o
    /// código do pipe de saída; `(código, saída, erro)`.
    pub(super) fn esperar_processo(entrada: i64, saida: i64, erro: i64, fim: i64) -> ResultadoIo<(i64, Vec<u8>, Vec<u8>)> {
        // SAFETY: fecha o handle da entrada do filho.
        unsafe { CloseHandle(tomar_handle(entrada)?) };
        let mut leitores = Vec::with_capacity(3);
        for fd in [saida, erro, fim] {
            leitores.push(LeituraDeEspera::nova(tomar_handle(fd)?)?);
        }
        let mut vivos: Vec<usize> = vec![0, 1, 2];
        while !vivos.is_empty() {
            let eventos: Vec<Bruto> = vivos.iter().map(|&i| leitores[i].evento).collect();
            // SAFETY: os eventos dos leitores vivos.
            let r = unsafe { WaitForMultipleObjects(eventos.len() as u32, eventos.as_ptr(), 0, INFINITE) };
            let k = r.wrapping_sub(WAIT_OBJECT_0) as usize;
            if k >= vivos.len() {
                return Err(erro_do_sistema());
            }
            if !leitores[vivos[k]].avancar()? {
                vivos.remove(k);
            }
        }
        let codigo = &leitores[2].dados;
        if codigo.len() != 8 {
            return Err(ErroDoSo { codigo: -1, mensagem: "Failed to read the process exit code".to_string() });
        }
        let valor = i64::from(i32::from_ne_bytes(codigo[..4].try_into().expect("4 bytes")));
        let negativo = i32::from_ne_bytes(codigo[4..].try_into().expect("4 bytes"));
        let mut l = leitores.into_iter();
        let out = std::mem::take(&mut l.next().expect("saída").dados);
        let err = std::mem::take(&mut l.next().expect("erro").dados);
        Ok((if negativo != 0 { -valor } else { valor }, out, err))
    }

    // -----------------------------------------------------------------------
    // Sinais (eventos de console).

    struct InscricaoDeSinal {
        evento: u32,
        /// O lado que o tratador escreve.
        escrita: Arc<Manipulador>,
        /// O descritor do lado que o Dart lê.
        leitura: i64,
    }

    fn inscricoes() -> std::sync::MutexGuard<'static, Vec<InscricaoDeSinal>> {
        static I: std::sync::Mutex<Vec<InscricaoDeSinal>> = std::sync::Mutex::new(Vec::new());
        I.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `GetWinSignal`: SIGHUP é o fechamento do console; SIGINT, o Ctrl+C.
    fn evento_de_console(sinal: i64) -> Option<u32> {
        match sinal {
            1 => Some(CTRL_CLOSE_EVENT),
            2 => Some(CTRL_C_EVENT),
            _ => None,
        }
    }

    /// `SignalHandler`: um byte no pipe de cada inscrição do evento.
    unsafe extern "system" fn tratar_console(evento: u32) -> i32 {
        let l = inscricoes();
        let mut tratado = false;
        for i in l.iter().filter(|i| i.evento == evento) {
            let _ = escrever_no_manipulador(&i.escrita, &[0]);
            tratado = true;
        }
        i32::from(tratado)
    }

    /// `Process::SetSignalHandler`: o descritor do pipe que o Dart lê.
    pub(super) fn inscrever_sinal(sinal: i64) -> ResultadoIo<i64> {
        let Some(evento) = evento_de_console(sinal) else {
            return Err(ErroDoSo::do_codigo(ERROR_NOT_SUPPORTED));
        };
        let [leitura, escrita] = criar_pipe(UsoDoPipe::Sinal)?;
        let escrita = Manipulador::novo(TipoDeManipulador::Arquivo, escrita);
        let leitura = Manipulador::novo(TipoDeManipulador::Arquivo, leitura);
        let mut l = inscricoes();
        // SAFETY: instala o tratador de console (uma vez para todas as
        // inscrições).
        if l.is_empty() && unsafe { SetConsoleCtrlHandler(Some(tratar_console), 1) } == 0 {
            return Err(erro_do_sistema());
        }
        let fd = descritor_de(leitura);
        l.push(InscricaoDeSinal { evento, escrita, leitura: fd });
        Ok(fd)
    }

    fn remover_inscricoes(filtro: impl Fn(&InscricaoDeSinal) -> bool) {
        let mut l = inscricoes();
        let antes = l.len();
        l.retain(|i| !filtro(i));
        if antes > 0 && l.is_empty() {
            // SAFETY: remove o tratador instalado por `inscrever_sinal`.
            unsafe { SetConsoleCtrlHandler(Some(tratar_console), 0) };
        }
    }

    /// `Process::ClearSignalHandler`.
    pub(super) fn cancelar_sinal(sinal: i64) {
        if let Some(evento) = evento_de_console(sinal) {
            remover_inscricoes(|i| i.evento == evento);
        }
    }

    /// `Process::ClearSignalHandlerByFd`: o soquete do sinal fechou.
    pub(super) fn limpar_sinal_por_descritor(fd: i64) {
        remover_inscricoes(|i| i.leitura == fd);
    }
}

#[cfg(windows)]
use processos_windows::*;
