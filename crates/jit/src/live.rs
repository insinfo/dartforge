//! Hot reload ao vivo: o programa roda numa thread própria enquanto a sessão
//! compila e publica gerações novas (R1b de `docs/PESQUISA-HOT-RELOAD.md`).
//!
//! ```text
//!   observer (who chama)            thread do program
//!   ───────────────────────            ──────────────────
//!   executar_main()  ── Executar ──►   main → laço de events (o isolate main_queue)
//!   publicar(ir):                          │
//!     analisa, liga a geração N            │   (o program continua servindo)
//!     pede o point secure  ─────────────►  ├─ entre dois events: a task roda —
//!                                          │  estáticos, células, records
//!     ◄──────────────── respondido ─────── │
//!   (o program segue, já no código new)  ▼
//! ```
//!
//! A publicação só roda com nenhum quadro Dart do isolado na pilha: entre dois
//! eventos do laço (`dartforge_pedir_no_ponto_seguro`, `portas.rs`), que é onde
//! a VM também comita uma recarga. Com o programa parado (o `main` terminou,
//! ou ainda não começou), ela roda na thread do programa entre dois comandos —
//! o estado do runtime (heap, estáticos, registros) é por thread, e a thread é
//! a mesma durante toda a sessão: um `main` executado de novo encontra o
//! estado da execução anterior.
//!
//! O `main` **não** é executado de novo por uma recarga: o programa em
//! execução passa a chamar o código novo a partir do próximo evento (um
//! timer, uma mensagem, uma conexão). Chamadas já em andamento terminam no
//! corpo em que entraram.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::ffi::{self, Task};
use crate::{ENTRY_SYMBOL, EntryReport, HotReloadReport, JitError, JitSession, PROGRAM_STACK_BYTES};

/// O que a thread do programa faz.
enum Command {
    /// Executa o programa pela entrada (`main` do SDK da fonte ou
    /// `dartforge_entry` do runtime embutido) e responde com o código de saída.
    Executar { address: u64, from_source: bool },
    /// Roda uma tarefa (a publicação com o programa parado) e confirma.
    Run(Task, mpsc::Sender<()>),
}

/// Um programa em execução na sessão, com a thread dele.
///
/// Obtido por [`JitSession::com_programa_vivo`], que garante que a thread do
/// programa termina antes de a sessão (e o código dela) deixar de existir.
pub struct ProgramaVivo<'s> {
    session: &'s mut JitSession,
    commands: mpsc::Sender<Command>,
    results: mpsc::Receiver<(i32, Duration)>,
    /// O programa foi mandado executar e ainda não respondeu.
    busy: Arc<AtomicBool>,
    /// Chamada quando o isolado demora a chegar ao ponto seguro.
    on_wait: Box<dyn FnMut() + 's>,
}

impl JitSession {
    /// Abre a thread do programa e entrega a `body` o [`ProgramaVivo`].
    ///
    /// Ao fim de `body`, a thread termina depois de o programa em execução
    /// terminar (o `main` e o laço de eventos dele): a sessão só é liberada
    /// sem código dela na pilha de ninguém.
    ///
    /// # Erros
    /// `execute` se a thread do programa não puder ser criada.
    pub fn com_programa_vivo<R>(
        &mut self,
        body: impl FnOnce(&mut ProgramaVivo<'_>) -> R,
    ) -> Result<R, JitError> {
        std::thread::scope(|scope| {
            let (commands, received_list) = mpsc::channel::<Command>();
            let (respond, results) = mpsc::channel::<(i32, Duration)>();
            let busy = Arc::new(AtomicBool::new(false));
            let thread_busy = busy.clone();
            std::thread::Builder::new()
                .name("dartforge-programa".into())
                .stack_size(PROGRAM_STACK_BYTES)
                .spawn_scoped(scope, move || {
                    for command in received_list {
                        match command {
                            Command::Executar { address, from_source } => {
                                let start = Instant::now();
                                let code = if from_source {
                                    ffi::call_source_main(address)
                                } else {
                                    ffi::call_embedded_entry(address)
                                };
                                thread_busy.store(false, Ordering::Release);
                                let _ = respond.send((code, start.elapsed()));
                            }
                            Command::Run(task, done) => {
                                task();
                                let _ = done.send(());
                            }
                        }
                    }
                })
                .map_err(|error| JitError::new("execute", "não foi possível criar a thread do programa", error.to_string()))?;
            let mut alive = ProgramaVivo { session: self, commands, results, busy, on_wait: Box::new(|| {}) };
            Ok(body(&mut alive))
            // `alive` sai de escopo aqui: sem o remetente dos comandos, a
            // thread termina quando o programa em execução terminar.
        })
    }
}

impl<'s> ProgramaVivo<'s> {
    /// A sessão, para consultas.
    pub fn session(&self) -> &JitSession {
        self.session
    }

    /// Define o que fazer quando a publicação espera mais de 2 s pelo ponto
    /// seguro (o isolado principal está num trecho síncrono longo).
    pub fn ao_esperar_ponto_seguro(&mut self, f: impl FnMut() + 's) {
        self.on_wait = Box::new(f);
    }

    /// O programa está executando (o `main` ou o laço de eventos dele).
    pub fn executando(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    /// Publica uma geração de `name`: a primeira pelo caminho de sempre
    /// ([`JitSession::add_reloadable_module`]); as seguintes no ponto seguro
    /// do programa em execução, ou direto na thread dele se estiver parado.
    ///
    /// # Erros
    /// Os de [`JitSession::hot_reload`]; a versão em execução continua
    /// publicada em todos eles.
    pub fn publicar(&mut self, name: &str, ir: &str) -> Result<HotReloadReport, JitError> {
        if self.session.generation(name).is_none() && !self.executando() {
            return self.session.add_reloadable_module(name, ir);
        }
        let runtime = match self.session.sdk_dll.clone() {
            Some(dll) => ffi::ReloadRuntime::from_library(&dll)
                .map_err(|detail| JitError::new("contract", "o runtime da sessão não publica gerações", detail))?,
            None => ffi::ReloadRuntime::embedded(),
        };
        let ProgramaVivo { session, commands, busy, on_wait, .. } = self;
        let mut at_safepoint = |task: Task| run_in_program(task, &runtime, commands, busy, on_wait.as_mut());
        session.hot_reload_at_safepoint(name, ir, &mut at_safepoint)
    }

    /// Manda o programa executar desde o `main`, sem esperar.
    ///
    /// # Erros
    /// `execute` se o programa já está executando; `lookup` se nenhuma
    /// geração publicou a entrada.
    pub fn executar_main(&mut self) -> Result<(), JitError> {
        if self.executando() {
            return Err(JitError::new("execute", "o programa já está executando", String::new()));
        }
        let from_source = self.session.usa_sdk_da_fonte();
        let input = self.session.stable_entry(if from_source { "main" } else { ENTRY_SYMBOL })?;
        let expected_one = if from_source { "i32 ()" } else { "void ()" };
        if input.signature() != expected_one {
            return Err(JitError::new(
                "execute",
                &format!("a entrada {} tem assinatura {}, esperada {expected_one}", input.name(), input.signature()),
                String::new(),
            ));
        }
        self.busy.store(true, Ordering::Release);
        self.commands
            .send(Command::Executar { address: input.address, from_source })
            .map_err(|_| JitError::new("execute", "a thread do programa terminou", String::new()))
    }

    /// O fim do programa, se já terminou (sem esperar).
    pub fn terminou(&mut self) -> Option<EntryReport> {
        let (exit_code, execute) = self.results.try_recv().ok()?;
        Some(EntryReport { lookup: Duration::ZERO, execute, total: execute, exit_code })
    }

    /// Espera o programa terminar.
    pub fn wait_end(&mut self) -> Option<EntryReport> {
        let (exit_code, execute) = self.results.recv().ok()?;
        Some(EntryReport { lookup: Duration::ZERO, execute, total: execute, exit_code })
    }
}

/// Roda `task` na thread do programa: no ponto seguro do isolado principal
/// se ele está no laço; senão, entre dois comandos, com o programa parado.
fn run_in_program(
    mut task: Task,
    runtime: &ffi::ReloadRuntime,
    commands: &mpsc::Sender<Command>,
    busy: &AtomicBool,
    on_wait: &mut dyn FnMut(),
) {
    loop {
        match runtime.at_safepoint(task, on_wait) {
            Ok(()) => return,
            Err(returned) => task = returned,
        }
        if busy.load(Ordering::Acquire) {
            // O programa foi mandado executar e ainda não marcou o isolado
            // principal (ou acabou de desmarcá-lo e vai responder): o ponto
            // seguro chega logo, ou o programa para.
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        let (done, wait_for) = mpsc::channel();
        if commands.send(Command::Run(task, done)).is_ok() {
            // Parado: a thread roda a tarefa assim que pegar o comando.
            let _ = wait_for.recv();
        }
        return;
    }
}
