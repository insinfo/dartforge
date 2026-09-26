//! Hot reload ao vivo: o programa roda numa thread própria enquanto a sessão
//! compila e publica gerações novas (R1b de `docs/PESQUISA-HOT-RELOAD.md`).
//!
//! ```text
//!   observador (quem chama)            thread do programa
//!   ───────────────────────            ──────────────────
//!   executar_main()  ── Executar ──►   main → laço de eventos (o isolado principal)
//!   publicar(ir):                          │
//!     analisa, liga a geração N            │   (o programa continua servindo)
//!     pede o ponto seguro  ─────────────►  ├─ entre dois eventos: a tarefa roda —
//!                                          │  estáticos, células, registros
//!     ◄──────────────── respondido ─────── │
//!   (o programa segue, já no código novo)  ▼
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

use crate::ffi::{self, Tarefa};
use crate::{ENTRY_SYMBOL, EntryReport, HotReloadReport, JitError, JitSession, PROGRAM_STACK_BYTES};

/// O que a thread do programa faz.
enum Comando {
    /// Executa o programa pela entrada (`main` do SDK da fonte ou
    /// `dartforge_entry` do runtime embutido) e responde com o código de saída.
    Executar { endereco: u64, da_fonte: bool },
    /// Roda uma tarefa (a publicação com o programa parado) e confirma.
    Rodar(Tarefa, mpsc::Sender<()>),
}

/// Um programa em execução na sessão, com a thread dele.
///
/// Obtido por [`JitSession::com_programa_vivo`], que garante que a thread do
/// programa termina antes de a sessão (e o código dela) deixar de existir.
pub struct ProgramaVivo<'s> {
    sessao: &'s mut JitSession,
    comandos: mpsc::Sender<Comando>,
    resultados: mpsc::Receiver<(i32, Duration)>,
    /// O programa foi mandado executar e ainda não respondeu.
    ocupado: Arc<AtomicBool>,
    /// Chamada quando o isolado demora a chegar ao ponto seguro.
    ao_esperar: Box<dyn FnMut() + 's>,
}

impl JitSession {
    /// Abre a thread do programa e entrega a `corpo` o [`ProgramaVivo`].
    ///
    /// Ao fim de `corpo`, a thread termina depois de o programa em execução
    /// terminar (o `main` e o laço de eventos dele): a sessão só é liberada
    /// sem código dela na pilha de ninguém.
    ///
    /// # Erros
    /// `execute` se a thread do programa não puder ser criada.
    pub fn com_programa_vivo<R>(
        &mut self,
        corpo: impl FnOnce(&mut ProgramaVivo<'_>) -> R,
    ) -> Result<R, JitError> {
        std::thread::scope(|escopo| {
            let (comandos, recebidos) = mpsc::channel::<Comando>();
            let (responder, resultados) = mpsc::channel::<(i32, Duration)>();
            let ocupado = Arc::new(AtomicBool::new(false));
            let ocupado_da_thread = ocupado.clone();
            std::thread::Builder::new()
                .name("dartforge-programa".into())
                .stack_size(PROGRAM_STACK_BYTES)
                .spawn_scoped(escopo, move || {
                    for comando in recebidos {
                        match comando {
                            Comando::Executar { endereco, da_fonte } => {
                                let inicio = Instant::now();
                                let codigo = if da_fonte {
                                    ffi::chamar_main_da_fonte(endereco)
                                } else {
                                    ffi::chamar_entrada_embutida(endereco)
                                };
                                ocupado_da_thread.store(false, Ordering::Release);
                                let _ = responder.send((codigo, inicio.elapsed()));
                            }
                            Comando::Rodar(tarefa, feito) => {
                                tarefa();
                                let _ = feito.send(());
                            }
                        }
                    }
                })
                .map_err(|erro| JitError::new("execute", "não foi possível criar a thread do programa", erro.to_string()))?;
            let mut vivo = ProgramaVivo { sessao: self, comandos, resultados, ocupado, ao_esperar: Box::new(|| {}) };
            Ok(corpo(&mut vivo))
            // `vivo` sai de escopo aqui: sem o remetente dos comandos, a
            // thread termina quando o programa em execução terminar.
        })
    }
}

impl<'s> ProgramaVivo<'s> {
    /// A sessão, para consultas.
    pub fn sessao(&self) -> &JitSession {
        self.sessao
    }

    /// Define o que fazer quando a publicação espera mais de 2 s pelo ponto
    /// seguro (o isolado principal está num trecho síncrono longo).
    pub fn ao_esperar_ponto_seguro(&mut self, f: impl FnMut() + 's) {
        self.ao_esperar = Box::new(f);
    }

    /// O programa está executando (o `main` ou o laço de eventos dele).
    pub fn executando(&self) -> bool {
        self.ocupado.load(Ordering::Acquire)
    }

    /// Publica uma geração de `nome`: a primeira pelo caminho de sempre
    /// ([`JitSession::add_reloadable_module`]); as seguintes no ponto seguro
    /// do programa em execução, ou direto na thread dele se estiver parado.
    ///
    /// # Erros
    /// Os de [`JitSession::hot_reload`]; a versão em execução continua
    /// publicada em todos eles.
    pub fn publicar(&mut self, nome: &str, ir: &str) -> Result<HotReloadReport, JitError> {
        if self.sessao.generation(nome).is_none() && !self.executando() {
            return self.sessao.add_reloadable_module(nome, ir);
        }
        let runtime = match self.sessao.sdk_dll.clone() {
            Some(dll) => ffi::RuntimeDaRecarga::da_biblioteca(&dll)
                .map_err(|detail| JitError::new("contract", "o runtime da sessão não publica gerações", detail))?,
            None => ffi::RuntimeDaRecarga::embutido(),
        };
        let ProgramaVivo { sessao, comandos, ocupado, ao_esperar, .. } = self;
        let mut no_ponto_seguro = |tarefa: Tarefa| rodar_no_programa(tarefa, &runtime, comandos, ocupado, ao_esperar.as_mut());
        sessao.hot_reload_no_ponto_seguro(nome, ir, &mut no_ponto_seguro)
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
        let da_fonte = self.sessao.usa_sdk_da_fonte();
        let entrada = self.sessao.stable_entry(if da_fonte { "main" } else { ENTRY_SYMBOL })?;
        let esperada = if da_fonte { "i32 ()" } else { "void ()" };
        if entrada.signature() != esperada {
            return Err(JitError::new(
                "execute",
                &format!("a entrada {} tem assinatura {}, esperada {esperada}", entrada.name(), entrada.signature()),
                String::new(),
            ));
        }
        self.ocupado.store(true, Ordering::Release);
        self.comandos
            .send(Comando::Executar { endereco: entrada.address, da_fonte })
            .map_err(|_| JitError::new("execute", "a thread do programa terminou", String::new()))
    }

    /// O fim do programa, se já terminou (sem esperar).
    pub fn terminou(&mut self) -> Option<EntryReport> {
        let (exit_code, execute) = self.resultados.try_recv().ok()?;
        Some(EntryReport { lookup: Duration::ZERO, execute, total: execute, exit_code })
    }

    /// Espera o programa terminar.
    pub fn esperar_fim(&mut self) -> Option<EntryReport> {
        let (exit_code, execute) = self.resultados.recv().ok()?;
        Some(EntryReport { lookup: Duration::ZERO, execute, total: execute, exit_code })
    }
}

/// Roda `tarefa` na thread do programa: no ponto seguro do isolado principal
/// se ele está no laço; senão, entre dois comandos, com o programa parado.
fn rodar_no_programa(
    mut tarefa: Tarefa,
    runtime: &ffi::RuntimeDaRecarga,
    comandos: &mpsc::Sender<Comando>,
    ocupado: &AtomicBool,
    ao_esperar: &mut dyn FnMut(),
) {
    loop {
        match runtime.no_ponto_seguro(tarefa, ao_esperar) {
            Ok(()) => return,
            Err(devolvida) => tarefa = devolvida,
        }
        if ocupado.load(Ordering::Acquire) {
            // O programa foi mandado executar e ainda não marcou o isolado
            // principal (ou acabou de desmarcá-lo e vai responder): o ponto
            // seguro chega logo, ou o programa para.
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        let (feito, esperar) = mpsc::channel();
        if comandos.send(Comando::Rodar(tarefa, feito)).is_ok() {
            // Parado: a thread roda a tarefa assim que pegar o comando.
            let _ = esperar.recv();
        }
        return;
    }
}
