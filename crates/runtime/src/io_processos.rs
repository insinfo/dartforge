// Runtime nativo: os processos do `dart:io` da VM (`runtime/bin/process.cc`,
// `process_linux.cc`, `process_macos.cc`): `Process.start`/`run`/`runSync`,
// `Process.killPid`, os sinais (`ProcessSignal.watch`) e o uso de memória
// (`ProcessInfo`).
//
// Como a VM: o filho nasce de `fork` + `execvp`, com pipes para a entrada,
// a saída e o erro (que o Dart lê como `_NativeSocket`, pelo manipulador de
// eventos) e um pipe de controle que devolve o `errno` de um `exec` que
// falhou; uma thread (`ExitCodeHandler`) espera os filhos com `wait` e
// escreve o código de saída (`[código, negativo]`, dois `int32`) no pipe de
// saída de cada processo. Os modos destacados passam por um segundo `fork`
// e `setsid`, e o neto informa o próprio pid.
//
// Entre o `fork` e o `exec`, o filho só chama funções seguras para sinais
// (`read`, `dup2`, `chdir`, `execvp`, `write`, `_exit`…); tudo o que ele usa
// (argumentos, ambiente) é montado antes do `fork`. O filho informa só o
// `errno`; a mensagem sai do mesmo `strerror` no pai.
//
// No Windows os processos ainda não são suportados: o início falha com o
// erro do sistema `ERROR_NOT_SUPPORTED`.

/// `ProcessStartMode`.
const MODO_NORMAL: i64 = 0;
const MODO_HERDAR_STDIO: i64 = 1;
const MODO_DESTACADO: i64 = 2;
const MODO_DESTACADO_COM_STDIO: i64 = 3;

fn modo_anexado(modo: i64) -> bool {
    modo == MODO_NORMAL || modo == MODO_HERDAR_STDIO
}

fn modo_com_stdio(modo: i64) -> bool {
    modo == MODO_NORMAL || modo == MODO_DESTACADO_COM_STDIO
}

/// Os textos de uma `List<String>` Dart em UTF-8; `None` se algum elemento
/// não é `String`.
fn textos_da_lista(lista: i64) -> Option<Vec<Vec<u8>>> {
    HEAP.with(|h| {
        let h = h.borrow();
        let n = h.list_len(lista);
        let mut saida = Vec::with_capacity(n);
        for i in 0..n {
            let v = h.list_get(lista, i);
            match (v.is_ref, h.try_get(v.bits)) {
                (true, Some(Value::String(t))) => saida.push(t.para_utf8_da_vm()),
                _ => return None,
            }
        }
        Some(saida)
    })
}

/// Grava o erro de início no `_ProcessStartStatus` (o `Dart_SetField` da
/// VM), pela função da sobreposição.
fn falha_ao_iniciar(status: i64, codigo: i64, mensagem: &str) {
    let Some(f) = ajudante("_dartforgeFalhaAoIniciar") else {
        panic!("bug do compilador: dart:io sem `_dartforgeFalhaAoIniciar` registrado");
    };
    // SAFETY: registrada pela sobreposição de `dart:io` com a assinatura
    // (`_ProcessStartStatus`, `int`, `String`) → `void`.
    let g: extern "C" fn(i64, i64, i64) = unsafe { std::mem::transmute(f) };
    let m = alocar_str(mensagem);
    com_raizes(&[m], || g(status, codigo, m));
}

/// Um processo iniciado: o pid e os descritores do lado do pai (-1 quando
/// o modo não tem).
struct ProcessoIniciado {
    pid: i64,
    entrada: i64,
    saida: i64,
    erro: i64,
    fim: i64,
}

// ---------------------------------------------------------------------------
// Unix.

#[cfg(unix)]
unsafe extern "C" {
    fn fork() -> i32;
    fn execvp(arquivo: *const std::ffi::c_char, argv: *const *const std::ffi::c_char) -> i32;
    fn dup2(de: i32, para: i32) -> i32;
    fn chdir(caminho: *const std::ffi::c_char) -> i32;
    fn setsid() -> i32;
    fn getpid() -> i32;
    fn _exit(codigo: i32) -> !;
    fn close(fd: i32) -> i32;
    fn open(caminho: *const std::ffi::c_char, flags: i32, ...) -> i32;
    fn kill(pid: i32, sinal: i32) -> i32;
    fn wait(status: *mut i32) -> i32;
    fn poll(fds: *mut PollFd, n: u64, espera: i32) -> i32;
    fn sysconf(nome: i32) -> std::ffi::c_long;
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn __errno_location() -> *mut i32;
    static mut environ: *const *const std::ffi::c_char;
}

#[cfg(all(unix, not(target_os = "linux")))]
unsafe extern "C" {
    fn __error() -> *mut i32;
    fn _NSGetEnviron() -> *mut *const *const std::ffi::c_char;
}

/// Troca o ambiente do processo (no filho, antes do `execvp`): o `environ`
/// da libc; no macOS, pelo `_NSGetEnviron` (o símbolo não é visível a uma
/// biblioteca dinâmica).
#[cfg(unix)]
unsafe fn trocar_ambiente(novo: *const *const std::ffi::c_char) {
    #[cfg(target_os = "linux")]
    // SAFETY: só o filho, de uma thread, antes do `exec`.
    unsafe {
        environ = novo;
    }
    #[cfg(not(target_os = "linux"))]
    // SAFETY: idem.
    unsafe {
        *_NSGetEnviron() = novo;
    }
}

/// O `errno` corrente (seguro no filho entre o `fork` e o `exec`).
#[cfg(unix)]
fn errno_atual() -> i32 {
    // SAFETY: o endereço do `errno` da thread.
    #[cfg(target_os = "linux")]
    unsafe {
        *__errno_location()
    }
    #[cfg(not(target_os = "linux"))]
    unsafe {
        *__error()
    }
}

#[cfg(unix)]
#[repr(C)]
struct PollFd {
    fd: i32,
    eventos: i16,
    reventos: i16,
}

/// Um pipe com os dois lados fechados no `exec` (`pipe2(O_CLOEXEC)`).
#[cfg(unix)]
fn pipe_fechado_no_exec() -> ResultadoIo<[i32; 2]> {
    let mut fds = [-1i32; 2];
    // SAFETY: `fds` tem dois inteiros.
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    fechar_no_exec(fds[0]);
    fechar_no_exec(fds[1]);
    Ok(fds)
}

/// Escreve tudo, repetindo na interrupção (`FDUtils::WriteToBlocking`).
#[cfg(unix)]
fn escrever_bloqueante(fd: i32, dados: &[u8]) -> isize {
    let mut feito = 0usize;
    while feito < dados.len() {
        // SAFETY: os dados são legíveis.
        let n = unsafe { write(fd, dados[feito..].as_ptr().cast(), dados.len() - feito) };
        if n < 0 {
            if errno_atual() == 4 {
                continue;
            }
            return -1;
        }
        if n == 0 {
            break;
        }
        feito += n as usize;
    }
    feito as isize
}

/// Lê até encher ou o fim (`FDUtils::ReadFromBlocking`).
#[cfg(unix)]
fn ler_bloqueante(fd: i32, destino: &mut [u8]) -> isize {
    let mut feito = 0usize;
    while feito < destino.len() {
        // SAFETY: o destino é gravável.
        let n = unsafe { read(fd, destino[feito..].as_mut_ptr().cast(), destino.len() - feito) };
        if n < 0 {
            if errno_atual() == 4 {
                continue;
            }
            return -1;
        }
        if n == 0 {
            break;
        }
        feito += n as usize;
    }
    feito as isize
}

/// Os filhos anexados e o pipe de saída de cada um (`ProcessInfoList`), e
/// a thread que os espera (`ExitCodeHandler`).
#[cfg(unix)]
struct EsperaDeFilhos {
    estado: std::sync::Mutex<EstadoDaEspera>,
    sinal: std::sync::Condvar,
}

#[cfg(unix)]
#[derive(Default)]
struct EstadoDaEspera {
    /// pid → lado de escrita do pipe de saída.
    ativos: std::collections::HashMap<i32, i32>,
    rodando: bool,
}

#[cfg(unix)]
fn espera_de_filhos() -> &'static EsperaDeFilhos {
    static E: std::sync::OnceLock<EsperaDeFilhos> = std::sync::OnceLock::new();
    E.get_or_init(|| EsperaDeFilhos { estado: std::sync::Mutex::new(EstadoDaEspera::default()), sinal: std::sync::Condvar::new() })
}

#[cfg(unix)]
impl EsperaDeFilhos {
    /// Registra um filho e garante a thread que espera.
    fn registrar(&'static self, pid: i32, fd_de_saida: i32) {
        let mut e = self.estado.lock().unwrap_or_else(|x| x.into_inner());
        e.ativos.insert(pid, fd_de_saida);
        self.sinal.notify_all();
        if e.rodando {
            return;
        }
        e.rodando = true;
        std::thread::Builder::new()
            .name("dart:io Process.start".to_string())
            .spawn(move || self.esperar())
            .expect("dart:io: falha ao criar a thread dos processos");
    }

    /// `ExitCodeHandlerEntry`: espera qualquer filho e informa o código.
    fn esperar(&self) {
        loop {
            {
                let mut e = self.estado.lock().unwrap_or_else(|x| x.into_inner());
                while e.ativos.is_empty() {
                    e = self.sinal.wait(e).unwrap_or_else(|x| x.into_inner());
                }
            }
            let mut status = 0i32;
            // SAFETY: espera um filho qualquer.
            let pid = unsafe { wait(&mut status) };
            if pid < 0 {
                if errno_atual() == 4 {
                    continue;
                }
                // Sem filhos (ECHILD): o registro está à frente do `fork`
                // de outro processo; tenta de novo.
                std::thread::sleep(std::time::Duration::from_millis(1));
                continue;
            }
            let (mut codigo, mut negativo) = (0i32, 0i32);
            let termino = status & 0x7f;
            if termino == 0 {
                codigo = (status >> 8) & 0xff;
            } else if termino != 0x7f {
                codigo = termino;
                negativo = 1;
            }
            let fd = self.estado.lock().unwrap_or_else(|x| x.into_inner()).ativos.remove(&pid);
            if let Some(fd) = fd {
                let mut msg = [0u8; 8];
                msg[..4].copy_from_slice(&codigo.to_ne_bytes());
                msg[4..].copy_from_slice(&negativo.to_ne_bytes());
                escrever_bloqueante(fd, &msg);
                // SAFETY: o lado de escrita é deste registro, fechado uma vez.
                unsafe { close(fd) };
            }
        }
    }
}

/// O `ProcessStarter` da VM.
#[cfg(unix)]
fn iniciar_processo(
    caminho: &[u8],
    argumentos: &[Vec<u8>],
    diretorio: Option<&[u8]>,
    ambiente: Option<&[Vec<u8>]>,
    modo: i64,
) -> Result<ProcessoIniciado, ErroDoSo> {
    let cstr = |b: &[u8]| std::ffi::CString::new(b.to_vec()).map_err(|_| ErroDoSo::do_codigo(codigo_do_so::INVALIDO));
    let caminho_c = cstr(caminho)?;
    let mut args_c = vec![caminho_c.clone()];
    for a in argumentos {
        args_c.push(cstr(a)?);
    }
    let mut argv: Vec<*const std::ffi::c_char> = args_c.iter().map(|c| c.as_ptr()).collect();
    argv.push(std::ptr::null());
    let ambiente_c = match ambiente {
        Some(v) => Some(v.iter().map(|e| cstr(e)).collect::<Result<Vec<_>, _>>()?),
        None => None,
    };
    let envp: Option<Vec<*const std::ffi::c_char>> = ambiente_c.as_ref().map(|v| {
        let mut p: Vec<_> = v.iter().map(|c| c.as_ptr()).collect();
        p.push(std::ptr::null());
        p
    });
    let diretorio_c = match diretorio {
        Some(d) => Some(cstr(d)?),
        None => None,
    };
    let nulo_c = c"/dev/null";

    let mut pipes: Vec<i32> = Vec::new();
    let fechar_todos = |pipes: &[i32]| {
        for &fd in pipes {
            if fd >= 0 {
                // SAFETY: descritores deste início.
                unsafe { close(fd) };
            }
        }
    };
    let controle = pipe_fechado_no_exec()?;
    pipes.extend(controle);
    let leitura_entrada = match pipe_fechado_no_exec() {
        Ok(p) => p,
        Err(e) => {
            fechar_todos(&pipes);
            return Err(e);
        }
    };
    pipes.extend(leitura_entrada);
    let (leitura_erro, escrita_saida) = if modo_com_stdio(modo) {
        let a = pipe_fechado_no_exec().inspect_err(|_| fechar_todos(&pipes))?;
        pipes.extend(a);
        let b = pipe_fechado_no_exec().inspect_err(|_| fechar_todos(&pipes))?;
        pipes.extend(b);
        (a, b)
    } else {
        ([-1, -1], [-1, -1])
    };
    // SAFETY: `sysconf(_SC_OPEN_MAX)`.
    let max_fds = {
        #[cfg(target_os = "linux")]
        const SC_OPEN_MAX: i32 = 4;
        #[cfg(not(target_os = "linux"))]
        const SC_OPEN_MAX: i32 = 5;
        let n = unsafe { sysconf(SC_OPEN_MAX) };
        if n < 0 { 256 } else { n as i32 }
    };

    // SAFETY: o filho só usa funções seguras para sinais e memória já
    // montada.
    let pid = unsafe { fork() };
    if pid < 0 {
        let e = ErroDoSo::de(&std::io::Error::last_os_error());
        fechar_todos(&pipes);
        return Err(e);
    }
    if pid == 0 {
        // --- O filho. ---
        // SAFETY: tudo aqui é seguro depois do `fork`.
        unsafe {
            let relatar = |fd: i32| -> ! {
                let e = errno_atual();
                escrever_bloqueante(fd, &e.to_ne_bytes());
                close(fd);
                _exit(1)
            };
            let mut msg = [0u8; 1];
            if ler_bloqueante(leitura_entrada[0], &mut msg) != 1 {
                _exit(1);
            }
            if modo_anexado(modo) {
                if modo == MODO_NORMAL {
                    if dup2(escrita_saida[0], 0) == -1 || dup2(leitura_entrada[1], 1) == -1 || dup2(leitura_erro[1], 2) == -1 {
                        relatar(controle[1]);
                    }
                }
                if let Some(d) = &diretorio_c {
                    if chdir(d.as_ptr()) != 0 {
                        relatar(controle[1]);
                    }
                }
                if let Some(e) = &envp {
                    trocar_ambiente(e.as_ptr());
                }
                execvp(caminho_c.as_ptr(), argv.as_ptr());
                relatar(controle[1]);
            }
            // Destacado: um segundo `fork` depois do `setsid`, e o neto
            // informa o próprio pid pelo pipe de controle.
            if modo == MODO_DESTACADO {
                close(leitura_entrada[0]);
                close(leitura_entrada[1]);
            }
            let p = fork();
            if p < 0 {
                relatar(controle[1]);
            }
            if p > 0 {
                _exit(0);
            }
            if setsid() == -1 {
                relatar(controle[1]);
            }
            let p = fork();
            if p < 0 {
                relatar(controle[1]);
            }
            if p > 0 {
                _exit(0);
            }
            if modo == MODO_DESTACADO {
                for fd in 0..max_fds {
                    if fd != controle[1] {
                        close(fd);
                    }
                }
                if open(nulo_c.as_ptr(), 2 /* O_RDWR */) != 0 || dup2(0, 1) != 1 || dup2(0, 2) != 2 {
                    relatar(controle[1]);
                }
            } else {
                for fd in 0..max_fds {
                    if fd != controle[1] && fd != escrita_saida[0] && fd != leitura_entrada[1] && fd != leitura_erro[1] {
                        close(fd);
                    }
                }
                if dup2(escrita_saida[0], 0) == -1 || dup2(leitura_entrada[1], 1) == -1 || dup2(leitura_erro[1], 2) == -1 {
                    relatar(controle[1]);
                }
                close(escrita_saida[0]);
                close(leitura_entrada[1]);
                close(leitura_erro[1]);
            }
            if let Some(d) = &diretorio_c {
                if chdir(d.as_ptr()) != 0 {
                    relatar(controle[1]);
                }
            }
            if let Some(e) = &envp {
                trocar_ambiente(e.as_ptr());
            }
            escrever_bloqueante(controle[1], &getpid().to_ne_bytes());
            execvp(caminho_c.as_ptr(), argv.as_ptr());
            relatar(controle[1]);
        }
    }

    // --- O pai. ---
    let mut fim = -1i64;
    if modo_anexado(modo) {
        match pipe_fechado_no_exec() {
            Ok(p) => {
                tornar_nao_bloqueante(p[0]);
                espera_de_filhos().registrar(pid, p[1]);
                fim = i64::from(p[0]);
            }
            Err(e) => {
                fechar_todos(&pipes);
                return Err(e);
            }
        }
    }
    let fechar_fim = |fim: i64| {
        if fim >= 0 {
            // SAFETY: o lado de leitura do pipe de saída deste início.
            unsafe { close(fim as i32) };
        }
    };
    if escrever_bloqueante(leitura_entrada[1], b"1") != 1 {
        let e = ErroDoSo::de(&std::io::Error::last_os_error());
        fechar_fim(fim);
        fechar_todos(&pipes);
        return Err(e);
    }
    // SAFETY: o lado de escrita do controle é só do filho agora.
    unsafe { close(controle[1]) };
    let mut pid_final = pid;
    let erro_do_filho = if modo_anexado(modo) {
        let mut b = [0u8; 4];
        match ler_bloqueante(controle[0], &mut b) {
            4 => Some(i32::from_ne_bytes(b)),
            -1 => Some(errno_atual()),
            _ => None,
        }
    } else {
        let mut b = [0u8; 8];
        match ler_bloqueante(controle[0], &mut b) {
            4 => {
                pid_final = i32::from_ne_bytes(b[..4].try_into().expect("4 bytes"));
                None
            }
            8 => {
                pid_final = i32::from_ne_bytes(b[..4].try_into().expect("4 bytes"));
                Some(i32::from_ne_bytes(b[4..].try_into().expect("4 bytes")))
            }
            -1 => Some(errno_atual()),
            _ => None,
        }
    };
    // SAFETY: o lado de leitura do controle, fechado uma vez.
    unsafe { close(controle[0]) };
    if let Some(e) = erro_do_filho {
        fechar_fim(fim);
        fechar_todos(&[leitura_entrada[0], leitura_entrada[1], leitura_erro[0], leitura_erro[1], escrita_saida[0], escrita_saida[1]]);
        return Err(ErroDoSo::do_codigo(e));
    }
    let mut r = ProcessoIniciado { pid: i64::from(pid_final), entrada: -1, saida: -1, erro: -1, fim };
    if modo_com_stdio(modo) {
        tornar_nao_bloqueante(leitura_entrada[0]);
        tornar_nao_bloqueante(escrita_saida[1]);
        tornar_nao_bloqueante(leitura_erro[0]);
        // A saída do filho é o que o pai lê (`_stdout`); a entrada, o que o
        // pai escreve (`_stdin`).
        r.saida = i64::from(leitura_entrada[0]);
        r.entrada = i64::from(escrita_saida[1]);
        r.erro = i64::from(leitura_erro[0]);
        // SAFETY: os lados do filho, fechados uma vez.
        unsafe {
            close(leitura_entrada[1]);
            close(escrita_saida[0]);
            close(leitura_erro[1]);
        }
    } else {
        // SAFETY: o pipe de sincronização, fechado uma vez.
        unsafe {
            close(leitura_entrada[0]);
            close(leitura_entrada[1]);
        }
    }
    Ok(r)
}

#[cfg(windows)]
fn iniciar_processo(
    _caminho: &[u8],
    _argumentos: &[Vec<u8>],
    _diretorio: Option<&[u8]>,
    _ambiente: Option<&[Vec<u8>]>,
    _modo: i64,
) -> Result<ProcessoIniciado, ErroDoSo> {
    Err(ErroDoSo::do_codigo(50 /* ERROR_NOT_SUPPORTED */))
}

/// `Process_Start(namespace, path, arguments, workingDirectory,
/// environment, mode, stdin, stdout, stderr, exitHandler, status)`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn dartforge_nativo_Process_Start(
    processo: i64,
    _ns: i64,
    caminho: i64,
    argumentos: i64,
    diretorio: i64,
    ambiente: i64,
    modo: i64,
    entrada: i64,
    saida: i64,
    erro: i64,
    fim: i64,
    status: i64,
) -> u8 {
    let caminho_b = utf8_de_texto(caminho);
    let Some(args) = textos_da_lista(argumentos) else {
        falha_ao_iniciar(status, 0, "Arguments must be builtin strings");
        return 0;
    };
    let dir = (diretorio != 0).then(|| utf8_de_texto(diretorio));
    let amb = if ambiente == 0 {
        None
    } else {
        match textos_da_lista(ambiente) {
            Some(v) => Some(v),
            None => {
                falha_ao_iniciar(status, 0, "Environment values must be builtin strings");
                return 0;
            }
        }
    };
    match iniciar_processo(&caminho_b, &args, dir.as_deref(), amb.as_deref(), modo.clamp(0, 3)) {
        Ok(r) => {
            if modo_com_stdio(modo) {
                definir_soquete_no_objeto(entrada, r.entrada, FinalizadorDeSoquete::Normal);
                definir_soquete_no_objeto(saida, r.saida, FinalizadorDeSoquete::Normal);
                definir_soquete_no_objeto(erro, r.erro, FinalizadorDeSoquete::Normal);
            }
            if modo_anexado(modo) {
                definir_soquete_no_objeto(fim, r.fim, FinalizadorDeSoquete::Normal);
            }
            gravar_campo_nativo(processo, r.pid);
            1
        }
        Err(e) => {
            falha_ao_iniciar(status, e.codigo, &e.mensagem);
            0
        }
    }
}

/// `Process::Wait` (o `runSync`): fecha a entrada, lê a saída e o erro até
/// o fim e o código do pipe de saída; `(código, saída, erro)`.
#[cfg(unix)]
fn esperar_processo(entrada: i64, saida: i64, erro: i64, fim: i64) -> ResultadoIo<(i64, Vec<u8>, Vec<u8>)> {
    fechar_descritor_de_soquete(entrada);
    let mut dados_saida = Vec::new();
    let mut dados_erro = Vec::new();
    let mut codigo = [0u8; 8];
    let mut vivos = vec![saida, erro, fim];
    const POLLIN: i16 = 1;
    const POLLERR: i16 = 8;
    const POLLHUP: i16 = 0x10;
    const POLLNVAL: i16 = 0x20;
    let fechar_vivos = |vivos: &[i64]| {
        for &fd in vivos {
            fechar_descritor_de_soquete(fd);
        }
    };
    while !vivos.is_empty() {
        let mut fds: Vec<PollFd> = vivos.iter().map(|&fd| PollFd { fd: fd as i32, eventos: POLLIN, reventos: 0 }).collect();
        // SAFETY: `fds` tem `len` elementos.
        let r = unsafe { poll(fds.as_mut_ptr(), fds.len() as u64, -1) };
        if r <= 0 {
            if r < 0 && errno_atual() == 4 {
                continue;
            }
            let e = ErroDoSo::de(&std::io::Error::last_os_error());
            fechar_vivos(&vivos);
            return Err(e);
        }
        let mut fechados = Vec::new();
        for f in &fds {
            let fd = i64::from(f.fd);
            if f.reventos & (POLLNVAL | POLLERR) != 0 {
                let e = ErroDoSo::de(&std::io::Error::last_os_error());
                fechar_vivos(&vivos);
                return Err(e);
            }
            if f.reventos & POLLIN != 0 {
                let disponivel = bytes_disponiveis(fd).max(0) as usize;
                if fd == fim {
                    if disponivel == 8 && ler_bloqueante(f.fd, &mut codigo) != 8 {
                        let e = ErroDoSo::de(&std::io::Error::last_os_error());
                        fechar_vivos(&vivos);
                        return Err(e);
                    }
                } else {
                    let mut b = vec![0u8; disponivel];
                    let n = ler_do_soquete(fd, &mut b)?;
                    b.truncate(n);
                    if fd == saida { dados_saida.extend(b) } else { dados_erro.extend(b) }
                }
            }
            if f.reventos & POLLHUP != 0 {
                fechar_descritor_de_soquete(fd);
                fechados.push(fd);
            }
        }
        vivos.retain(|fd| !fechados.contains(fd));
    }
    let valor = i64::from(i32::from_ne_bytes(codigo[..4].try_into().expect("4 bytes")));
    let negativo = i32::from_ne_bytes(codigo[4..].try_into().expect("4 bytes"));
    Ok((if negativo != 0 { -valor } else { valor }, dados_saida, dados_erro))
}

/// `Process_Wait(stdin, stdout, stderr, exitHandler)`: `[pid, código,
/// saída, erro]`; num erro, mata o filho e lança o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_Wait(processo: i64, entrada: i64, saida: i64, erro: i64, fim: i64) -> i64 {
    let (Some(e), Some(s), Some(r), Some(f)) = (soquete_do_objeto(entrada), soquete_do_objeto(saida), soquete_do_objeto(erro), soquete_do_objeto(fim)) else {
        return 0;
    };
    let pid = campo_nativo(processo);
    #[cfg(unix)]
    let resultado = {
        let fds = (e.descritor(), s.descritor(), r.descritor(), f.descritor());
        // Os descritores passam a este `wait`, que os fecha.
        for x in [e, s, r, f] {
            x.descritor.store(-1, std::sync::atomic::Ordering::Release);
        }
        esperar_processo(fds.0, fds.1, fds.2, fds.3)
    };
    #[cfg(windows)]
    let resultado: ResultadoIo<(i64, Vec<u8>, Vec<u8>)> = {
        let _ = (e, s, r, f);
        Err(ErroDoSo::do_codigo(50))
    };
    match resultado {
        Ok((codigo, out, err)) => {
            let o = dart_bytes(out);
            let x = com_raizes(&[o], || dart_bytes(err));
            com_raizes(&[o, x], || dart_lista_fixa(&[dart_int(pid), dart_int(codigo), o, x]))
        }
        Err(erro) => {
            matar_processo(pid, 9);
            lancar_os_error(&erro);
            0
        }
    }
}

#[cfg(unix)]
fn matar_processo(pid: i64, sinal: i64) -> bool {
    loop {
        // SAFETY: envia um sinal a um pid.
        if unsafe { kill(pid as i32, sinal as i32) } != -1 {
            return true;
        }
        if errno_atual() != 4 {
            return false;
        }
    }
}
#[cfg(windows)]
fn matar_processo(_pid: i64, _sinal: i64) -> bool {
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_KillPid(pid: i64, sinal: i64) -> u8 {
    u8::from(matar_processo(pid, sinal))
}

// ---------------------------------------------------------------------------
// Sinais (`ProcessSignal.watch`): cada inscrição é um pipe; o tratador do
// sinal escreve um byte nos pipes do sinal, e o Dart lê o outro lado como
// soquete.

/// Os sinais que o Dart pode vigiar (`kSignals`).
#[cfg(unix)]
fn sinal_vigiavel(s: i64) -> bool {
    #[cfg(target_os = "linux")]
    const SINAIS: &[i64] = &[1, 2, 15, 10, 12, 28, 3];
    #[cfg(not(target_os = "linux"))]
    const SINAIS: &[i64] = &[1, 2, 15, 30, 31, 28, 3];
    SINAIS.contains(&s)
}

/// As inscrições: (sinal, lado de escrita do pipe); o tratador só lê
/// atômicos.
#[cfg(unix)]
const MAXIMO_DE_INSCRICOES: usize = 64;
#[cfg(unix)]
static INSCRICOES: [(std::sync::atomic::AtomicI32, std::sync::atomic::AtomicI32); MAXIMO_DE_INSCRICOES] =
    [const { (std::sync::atomic::AtomicI32::new(0), std::sync::atomic::AtomicI32::new(-1)) }; MAXIMO_DE_INSCRICOES];

#[cfg(unix)]
extern "C" fn tratar_sinal(sinal: i32) {
    for (s, fd) in &INSCRICOES {
        if s.load(std::sync::atomic::Ordering::Acquire) == sinal {
            let fd = fd.load(std::sync::atomic::Ordering::Acquire);
            if fd >= 0 {
                let b = [0u8];
                // SAFETY: `write` é seguro num tratador de sinal.
                unsafe { write(fd, b.as_ptr().cast(), 1) };
            }
        }
    }
}

/// `signal(sinal, tratador)`: o tratador anterior.
#[cfg(unix)]
unsafe extern "C" {
    fn signal(sinal: i32, tratador: usize) -> usize;
}

#[cfg(unix)]
fn inscricoes_mutex() -> std::sync::MutexGuard<'static, std::collections::HashMap<i32, usize>> {
    // Sinal → o tratador anterior, restaurado quando a última inscrição sai.
    static M: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<i32, usize>>> = std::sync::OnceLock::new();
    M.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// `Process::SetSignalHandler`: o lado de leitura do pipe da inscrição.
#[cfg(unix)]
fn inscrever_sinal(sinal: i64) -> ResultadoIo<i64> {
    if !sinal_vigiavel(sinal) {
        return Err(ErroDoSo::do_codigo(codigo_do_so::INVALIDO));
    }
    let fds = pipe_fechado_no_exec()?;
    let mut anteriores = inscricoes_mutex();
    let Some(livre) = INSCRICOES.iter().find(|(_, fd)| fd.load(std::sync::atomic::Ordering::Acquire) < 0) else {
        // SAFETY: os descritores acabaram de ser criados.
        unsafe {
            close(fds[0]);
            close(fds[1]);
        }
        return Err(ErroDoSo::do_codigo(24 /* EMFILE */));
    };
    livre.0.store(sinal as i32, std::sync::atomic::Ordering::Release);
    livre.1.store(fds[1], std::sync::atomic::Ordering::Release);
    if let std::collections::hash_map::Entry::Vacant(v) = anteriores.entry(sinal as i32) {
        // SAFETY: instala o tratador do sinal.
        let antigo = unsafe { signal(sinal as i32, tratar_sinal as usize) };
        v.insert(antigo);
    }
    Ok(i64::from(fds[0]))
}

/// `Process::ClearSignalHandler`: tira as inscrições do sinal e restaura o
/// tratador anterior.
#[cfg(unix)]
fn cancelar_sinal(sinal: i64) {
    let mut anteriores = inscricoes_mutex();
    for (s, fd) in &INSCRICOES {
        if s.load(std::sync::atomic::Ordering::Acquire) == sinal as i32 {
            let f = fd.swap(-1, std::sync::atomic::Ordering::AcqRel);
            s.store(0, std::sync::atomic::Ordering::Release);
            if f >= 0 {
                // SAFETY: o lado de escrita da inscrição, fechado uma vez.
                unsafe { close(f) };
            }
        }
    }
    if let Some(antigo) = anteriores.remove(&(sinal as i32)) {
        // SAFETY: restaura o tratador anterior.
        unsafe { signal(sinal as i32, antigo) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_SetSignalHandler(sinal: i64) -> i64 {
    #[cfg(unix)]
    {
        dart_ou_erro(inscrever_sinal(sinal), dart_int)
    }
    #[cfg(windows)]
    {
        let _ = sinal;
        ErroDoSo::do_codigo(50).para_dart()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Process_ClearSignalHandler(sinal: i64) {
    #[cfg(unix)]
    cancelar_sinal(sinal);
    #[cfg(windows)]
    let _ = sinal;
}

// ---------------------------------------------------------------------------
// `ProcessInfo`: memória residente.

/// `Process::CurrentRSS`.
#[cfg(target_os = "linux")]
fn memoria_residente() -> ResultadoIo<i64> {
    unsafe extern "C" {
        fn getpagesize() -> i32;
    }
    let statm = std::fs::read_to_string("/proc/self/statm")?;
    let paginas: i64 = statm.split_whitespace().nth(1).and_then(|x| x.parse().ok()).ok_or_else(|| ErroDoSo::do_codigo(codigo_do_so::INVALIDO))?;
    // SAFETY: consulta sem efeitos.
    Ok(paginas * i64::from(unsafe { getpagesize() }))
}
#[cfg(all(unix, not(target_os = "linux")))]
fn memoria_residente() -> ResultadoIo<i64> {
    #[repr(C)]
    #[derive(Default)]
    struct InfoBasica {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time: [i32; 2],
        system_time: [i32; 2],
        policy: i32,
        suspend_count: i32,
    }
    unsafe extern "C" {
        static mach_task_self_: u32;
        fn task_info(tarefa: u32, sabor: i32, info: *mut InfoBasica, n: *mut u32) -> i32;
    }
    let mut i = InfoBasica::default();
    let mut n = (std::mem::size_of::<InfoBasica>() / 4) as u32;
    // SAFETY: MACH_TASK_BASIC_INFO = 20; `i` tem o tamanho informado.
    if unsafe { task_info(mach_task_self_, 20, &mut i, &mut n) } != 0 {
        return Err(ErroDoSo::do_codigo(codigo_do_so::INVALIDO));
    }
    Ok(i.resident_size as i64)
}

/// `Process::MaxRSS` (`getrusage`: KiB no Linux, bytes no macOS).
#[cfg(unix)]
fn memoria_residente_maxima() -> ResultadoIo<i64> {
    #[repr(C)]
    struct Uso {
        tempos: [i64; 4],
        maxrss: std::ffi::c_long,
        resto: [std::ffi::c_long; 13],
    }
    unsafe extern "C" {
        fn getrusage(quem: i32, uso: *mut Uso) -> i32;
    }
    // SAFETY: a estrutura é só dados; a função a preenche (RUSAGE_SELF = 0).
    let mut u: Uso = unsafe { std::mem::zeroed() };
    if unsafe { getrusage(0, &mut u) } < 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    Ok(if cfg!(target_os = "linux") { u.maxrss as i64 * 1024 } else { u.maxrss as i64 })
}

#[cfg(windows)]
fn contadores_de_memoria() -> ResultadoIo<(i64, i64)> {
    #[repr(C)]
    #[derive(Default)]
    struct Contadores {
        cb: u32,
        faltas: u32,
        pico: usize,
        atual: usize,
        resto: [usize; 6],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(p: *mut std::ffi::c_void, c: *mut Contadores, n: u32) -> i32;
    }
    let mut c = Contadores { cb: std::mem::size_of::<Contadores>() as u32, ..Default::default() };
    // SAFETY: `c` tem o tamanho informado.
    if unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) } == 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    Ok((c.atual as i64, c.pico as i64))
}
#[cfg(windows)]
fn memoria_residente() -> ResultadoIo<i64> {
    contadores_de_memoria().map(|c| c.0)
}
#[cfg(windows)]
fn memoria_residente_maxima() -> ResultadoIo<i64> {
    contadores_de_memoria().map(|c| c.1)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ProcessInfo_CurrentRSS() -> i64 {
    dart_ou_erro(memoria_residente(), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ProcessInfo_MaxRSS() -> i64 {
    dart_ou_erro(memoria_residente_maxima(), dart_int)
}
