//! Live hot reload: the program runs on its own thread while the session compiles
//! and publishes new generations (R1b of `docs/PESQUISA-HOT-RELOAD.md`).
//!
//! ```text
//!   observer (caller)                  program thread
//!   ───────────────────────            ──────────────────
//!   execute_main()  ── Run ────────►   main → event loop (the main isolate)
//!   publish(ir):                           │
//!     analyzes, links generation N         │   (the program keeps serving)
//!     requests the safepoint  ──────────►  ├─ between two events: the task runs —
//!                                          │  statics, cells, records
//!     ◄──────────────── answered ───────── │
//!   (the program goes on, already in the new code)  ▼
//! ```
//!
//! Publication only runs with no Dart frame of the isolate on the stack: between
//! two events of the loop (`dartforge_request_at_safepoint`, `ports.rs`), which
//! is where the VM also commits a reload. With the program stopped (`main` has
//! finished, or has not started yet), it runs on the program thread between two
//! commands — the runtime state (heap, statics, registrations) is per thread, and
//! the thread is the same during the whole session: a `main` run again finds the
//! state of the previous run.
//!
//! `main` is **not** run again by a reload: the running program starts calling
//! the new code from the next event on (a timer, a message, a connection). Calls
//! already in progress finish in the body they entered.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::ffi::{self, Task};
use crate::{ENTRY_SYMBOL, EntryReport, HotReloadReport, JitError, JitSession, PROGRAM_STACK_BYTES};

/// What the program thread does.
enum Command {
    /// Runs the program through its entry (`main` of the SDK from source or
    /// `dartforge_entry` of the embedded runtime) and answers with the exit code.
    Executar { address: u64, from_source: bool },
    /// Runs a task (publication with the program stopped) and confirms.
    Run(Task, mpsc::Sender<()>),
}

/// A program running in the session, with its thread.
///
/// Obtained through [`JitSession::com_programa_vivo`], which guarantees that the
/// program thread ends before the session (and its code) stops existing.
pub struct ProgramaVivo<'s> {
    session: &'s mut JitSession,
    commands: mpsc::Sender<Command>,
    results: mpsc::Receiver<(i32, Duration)>,
    /// The program was told to run and has not answered yet.
    busy: Arc<AtomicBool>,
    /// Called when the isolate takes long to reach the safepoint.
    on_wait: Box<dyn FnMut() + 's>,
}

impl JitSession {
    /// Opens the program thread and hands the [`ProgramaVivo`] to `body`.
    ///
    /// When `body` ends, the thread ends after the running program ends (its
    /// `main` and its event loop): the session is only released with none of its
    /// code on anyone's stack.
    ///
    /// # Errors
    /// `execute` if the program thread cannot be created.
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
                .map_err(|error| JitError::new("execute", "could not create the program thread", error.to_string()))?;
            let mut alive = ProgramaVivo { session: self, commands, results, busy, on_wait: Box::new(|| {}) };
            Ok(body(&mut alive))
            // `alive` goes out of scope here: without the command sender, the
            // thread ends when the running program ends.
        })
    }
}

impl<'s> ProgramaVivo<'s> {
    /// The session, for queries.
    pub fn session(&self) -> &JitSession {
        self.session
    }

    /// Sets what to do when publication waits more than 2 s for the safepoint
    /// (the main isolate is in a long synchronous stretch).
    pub fn ao_esperar_ponto_seguro(&mut self, f: impl FnMut() + 's) {
        self.on_wait = Box::new(f);
    }

    /// The program is running (its `main` or its event loop).
    pub fn executando(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    /// Publishes a generation of `name`: the first one through the usual path
    /// ([`JitSession::add_reloadable_module`]); the following ones at the running
    /// program's safepoint, or directly on its thread if it is stopped.
    ///
    /// # Errors
    /// Those of [`JitSession::hot_reload`]; the running version stays published
    /// in all of them.
    pub fn publicar(&mut self, name: &str, ir: &str) -> Result<HotReloadReport, JitError> {
        if self.session.generation(name).is_none() && !self.executando() {
            return self.session.add_reloadable_module(name, ir);
        }
        let runtime = match self.session.sdk_dll.clone() {
            Some(dll) => ffi::ReloadRuntime::from_library(&dll)
                .map_err(|detail| JitError::new("contract", "the session's runtime does not publish generations", detail))?,
            None => ffi::ReloadRuntime::embedded(),
        };
        let ProgramaVivo { session, commands, busy, on_wait, .. } = self;
        let mut at_safepoint = |task: Task| run_in_program(task, &runtime, commands, busy, on_wait.as_mut());
        session.hot_reload_at_safepoint(name, ir, &mut at_safepoint)
    }

    /// Tells the program to run from `main`, without waiting.
    ///
    /// # Errors
    /// `execute` if the program is already running; `lookup` if no generation
    /// published the entry.
    pub fn executar_main(&mut self) -> Result<(), JitError> {
        if self.executando() {
            return Err(JitError::new("execute", "the program is already running", String::new()));
        }
        let from_source = self.session.usa_sdk_da_fonte();
        let input = self.session.stable_entry(if from_source { "main" } else { ENTRY_SYMBOL })?;
        let expected_one = if from_source { "i32 ()" } else { "void ()" };
        if input.signature() != expected_one {
            return Err(JitError::new(
                "execute",
                &format!("the entry {} has signature {}, expected {expected_one}", input.name(), input.signature()),
                String::new(),
            ));
        }
        self.busy.store(true, Ordering::Release);
        self.commands
            .send(Command::Executar { address: input.address, from_source })
            .map_err(|_| JitError::new("execute", "the program thread has ended", String::new()))
    }

    /// The end of the program, if it has already ended (without waiting).
    pub fn terminou(&mut self) -> Option<EntryReport> {
        let (exit_code, execute) = self.results.try_recv().ok()?;
        Some(EntryReport { lookup: Duration::ZERO, execute, total: execute, exit_code })
    }

    /// Waits for the program to end.
    pub fn wait_end(&mut self) -> Option<EntryReport> {
        let (exit_code, execute) = self.results.recv().ok()?;
        Some(EntryReport { lookup: Duration::ZERO, execute, total: execute, exit_code })
    }
}

/// Runs `task` on the program thread: at the main isolate's safepoint if it is in
/// the loop; otherwise, between two commands, with the program stopped.
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
            // The program was told to run and has not marked the main isolate yet
            // (or has just unmarked it and is about to answer): the safepoint
            // comes soon, or the program stops.
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        let (done, wait_for) = mpsc::channel();
        if commands.send(Command::Run(task, done)).is_ok() {
            // Stopped: the thread runs the task as soon as it takes the command.
            let _ = wait_for.recv();
        }
        return;
    }
}
