//! Execução em memória do LLVM IR do backend nativo, via LLVM ORCv2 (`LLJIT`).
//!
//! Este é o **perfil de desenvolvimento** do DartForge. O perfil de produção
//! é o AOT de `crates/emit_native`: IR → objeto por Clang → executável ligado
//! ao runtime Rust. Os dois consomem **exatamente o mesmo LLVM IR textual**; o
//! que muda é só o destino dele:
//!
//! | | AOT (`crates/emit_native`) | JIT (este crate) |
//! | --- | --- | --- |
//! | Geração de código | Clang `-O0`, em processo separado | ORCv2 em memória, `CodeGenLevelNone`, CPU `x86-64` |
//! | Runtime | `rustc` compila `runtime_main.rs` e o linker resolve | a mesma fonte, compilada neste crate; endereços publicados como símbolos absolutos |
//! | `main` | o `main` C do harness chama `@dartforge_entry` | o mesmo `main` do harness, chamado numa thread própria |
//! | Saída | executável em disco | nada em disco |
//!
//! O contrato completo — de onde vem cada símbolo, a pré-verificação de
//! externos, o que é provisório — está em `docs/JIT.md` e no plano do JIT.
//!
//! # Término do programa
//!
//! O runtime encerra o **processo** com `process::exit` nos mesmos casos em que
//! o executável AOT termina: exceção não capturada e asserção de não nulidade
//! (101), teto do heap (255). Um erro interno do runtime aborta. Dentro de
//! `dartforge run` isso é o comportamento certo — o processo termina com o
//! código do programa, como `dart run`. Para testes e para o harness
//! diferencial, a execução isolada é o binário `dartforge-executar-ir`.
//!
//! # Exemplo
//!
//! ```no_run
//! let ir = "\
//! declare void @dartforge_print_i64(i64)
//! define void @dartforge_entry() {
//!   call void @dartforge_print_i64(i64 7)
//!   ret void
//! }
//! ";
//! let relatorio = dartforge_jit::run_ir(ir)?; // imprime 7 no stdout
//! assert_eq!(relatorio.entry.exit_code, 0);
//! # Ok::<(), dartforge_jit::JitError>(())
//! ```
//!
//! # Requisito de build e de execução
//!
//! O crate usa `llvm-sys` 221, que localiza o LLVM 22.1.x pelo `llvm-config` da
//! distribuição completa; `LLVM_SYS_221_PREFIX` aponta o prefixo, e
//! `.cargo/config.toml` já a define para o repositório. A ligação é **dinâmica**,
//! contra `LLVM-C.dll`, porque as bibliotecas estáticas do pacote oficial de
//! Windows usam CRT estática e conflitam com a CRT dinâmica do Rust. Logo a DLL
//! precisa estar alcançável pelo carregador em tempo de execução;
//! `scripts/env.ps1` cuida disso. Veja [`docs/JIT.md`].
//!
//! [`docs/JIT.md`]: https://github.com/insinfo/dartforge/blob/main/docs/JIT.md
mod ffi;
mod reload;

pub use reload::{HotReloadReport, StableEntry};

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Nome do símbolo que o emissor nativo gera para o programa.
///
/// O emissor não produz uma função chamada `main`: o registro das classes e a
/// chamada ao `main` Dart ficam em `void @dartforge_entry()`. Quem procurar
/// `main` por engano recebe erro de símbolo ausente, não uma chamada errada.
pub const ENTRY_SYMBOL: &str = "dartforge_entry";

/// Nomes do runtime nativo que a sessão publica para o código gerado.
///
/// Gerado por `build.rs` a partir dos `#[unsafe(no_mangle)]` de
/// `crates/runtime/src/runtime_main.rs` — a mesma fonte que o AOT liga —, na
/// ordem da fonte. Nenhum nome é mantido à mão.
///
/// ```
/// assert!(dartforge_jit::RUNTIME_SYMBOLS.contains(&"dartforge_print_i64"));
/// assert!(!dartforge_jit::RUNTIME_SYMBOLS.contains(&"dartforge_entry"));
/// ```
pub const RUNTIME_SYMBOLS: &[&str] = ffi::RUNTIME_SYMBOLS;

/// Externos da CRT que o IR pode declarar além do runtime.
///
/// Lista fechada: qualquer outro nome declarado e não definido por um módulo da
/// sessão é recusado na etapa `símbolos` antes de chegar ao LLVM.
pub const CRT_SYMBOLS: &[&str] = ffi::CRT_SYMBOLS;

/// Pilha da thread que executa o programa, em bytes.
///
/// É a reserva padrão que o linker dá à thread principal de um executável
/// Windows — a que o programa AOT recebe, já que o driver não passa `/STACK`.
/// Usar o mesmo tamanho faz recursão profunda falhar nos mesmos pontos nos dois
/// perfis. Se o AOT passar a fixar a pilha, esta constante acompanha.
pub const PROGRAM_STACK_BYTES: usize = 1 << 20;

/// Etapa e diagnóstico de uma falha da sessão JIT.
///
/// `message` traz a explicação em português e, quando existe, o texto original
/// do LLVM anexado. Nenhum caminho de erro esperado usa `panic`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JitError {
    /// Etapa que falhou: `lljit`, `runtime-symbols`, `parse-ir`, `layout`,
    /// `símbolos`, `add-module`, `lookup`, `execute`, `resource` ou, no hot
    /// reload, `contract`, `link`, `publish` e `poisoned`.
    pub stage: &'static str,
    /// Explicação em português, com o diagnóstico do LLVM quando houver.
    pub message: String,
}
impl std::fmt::Display for JitError {
    /// Apresenta a etapa antes da mensagem.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.stage, self.message)
    }
}
impl std::error::Error for JitError {}
impl JitError {
    /// Monta o erro anexando o diagnóstico original do LLVM à explicação.
    fn new(stage: &'static str, explanation: &str, detail: String) -> Self {
        Self {
            stage,
            message: if detail.is_empty() {
                explanation.to_owned()
            } else {
                format!("{explanation}: {detail}")
            },
        }
    }
}

/// Custo de incorporar um módulo à sessão, medido por fase.
///
/// Cada intervalo é cronometrado no próprio trecho, nunca por subtração, no
/// mesmo espírito de `docs/DESEMPENHO.md`. `total` cobre a chamada inteira e é
/// pelo menos a soma das fases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleReport {
    /// Análise do IR textual, verificações e construção do `ThreadSafeModule`.
    pub parse_ir: Duration,
    /// Entrega do módulo à `LLJIT` sob um `ResourceTracker` próprio.
    pub add_module: Duration,
    /// Chamada completa de [`JitSession::add_ir_module`].
    pub total: Duration,
    /// Bytes de IR textual analisados, para separar tempo de tamanho.
    pub ir_bytes: usize,
}

/// Custo e resultado de resolver e executar a entrada da sessão.
///
/// `lookup` inclui a **geração de código sob demanda**: no ORCv2 o módulo só é
/// compilado quando algum de seus símbolos é procurado. Por isso o primeiro
/// `lookup` de uma sessão costuma dominar o relatório, e o `execute` seguinte
/// mede apenas a execução do código já materializado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryReport {
    /// Resolução do símbolo, incluindo a compilação sob demanda do módulo.
    pub lookup: Duration,
    /// Execução do programa, do `main` do harness ao retorno.
    pub execute: Duration,
    /// Chamada completa de [`JitSession::run_entry`].
    pub total: Duration,
    /// Código que o `main` do harness devolveu. Os finais com código diferente
    /// de zero encerram o processo antes de chegar aqui (ver o topo do crate).
    pub exit_code: i32,
}

/// Custo completo de compilar e executar um IR numa sessão descartável.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JitReport {
    /// Criação da `LLJIT` e definição dos símbolos de runtime.
    pub session: Duration,
    /// Fases de incorporação do módulo.
    pub module: ModuleReport,
    /// Fases de resolução e execução.
    pub entry: EntryReport,
    /// Chamada completa, incluindo a destruição da sessão.
    pub total: Duration,
}

/// Uma `LLJIT` viva, com o runtime nativo já resolvível pelo código gerado.
///
/// # Threading
///
/// A sessão não é `Send` nem `Sync`: contém ponteiros crus da API C. O programa
/// executa numa thread própria por execução ([`JitSession::run_entry`]), com o
/// runtime zerado, porque todo o estado do runtime é `thread_local`.
///
/// # Símbolos duplicados
///
/// Todos os módulos entram na mesma `JITDylib`, onde cada nome é definido uma
/// única vez. Como todo programa Dart compilado para o alvo nativo define
/// `@dartforge_entry`, **dois programas completos não cabem na mesma sessão**:
/// o segundo `add_ir_module` devolve erro de definição duplicada. Use uma sessão
/// por programa, ou módulos com símbolos distintos.
pub struct JitSession {
    /// Rastreadores dos módulos simples, na ordem em que foram adicionados.
    ///
    /// Declarado **antes** de `lljit` de propósito: os campos são destruídos na
    /// ordem de declaração, e liberar um `ResourceTracker` depois de destruir a
    /// `LLJIT` que o criou seria uso de memória liberada. O mesmo vale para
    /// `reloadables`, que também guarda rastreadores.
    modules: Vec<Module>,
    /// Módulos recarregáveis, com as gerações retidas e os trampolins.
    reloadables: Vec<reload::Reloadable>,
    /// Motivo pelo qual a sessão parou de aceitar recargas, se houver.
    poisoned: Option<String>,
    /// Identidade da sessão, para que uma [`StableEntry`] não cruze sessões.
    id: u64,
    lljit: ffi::Lljit,
}

/// Um módulo já incorporado à sessão e o rastreador que o descarrega.
struct Module {
    name: String,
    tracker: ffi::ResourceTracker,
    removed: bool,
    /// Assinaturas das funções que ele define, para a verificação de contrato.
    signatures: Vec<ffi::FunctionSignature>,
    /// Layout nominal das classes que ele constrói: `(class_id, campos)`.
    layouts: Vec<(i64, i64)>,
}

impl JitSession {
    /// Abre uma `LLJIT` para o host e publica os símbolos do runtime nativo.
    ///
    /// Os símbolos vêm de [`RUNTIME_SYMBOLS`] e são registrados como endereços
    /// absolutos das funções Rust deste processo — não pelo gerador de símbolos
    /// do processo (`LLVMOrcCreateDynamicLibrarySearchGeneratorForProcess`), por
    /// duas razões: um executável Windows não exporta os `#[no_mangle]` das
    /// bibliotecas Rust que liga, então o gerador não os acharia; e ele
    /// resolveria por acidente um nome homônimo em vez de falhar alto.
    ///
    /// # Erros
    /// Falha se o LLVM não tiver backend para o host, se a `LLJIT` não puder ser
    /// construída ou se algum símbolo de runtime já estiver definido.
    ///
    /// ```no_run
    /// let sessao = dartforge_jit::JitSession::new()?;
    /// assert!(sessao.module_names().is_empty());
    /// # Ok::<(), dartforge_jit::JitError>(())
    /// ```
    pub fn new() -> Result<Self, JitError> {
        let lljit = ffi::Lljit::new()
            .map_err(|detail| JitError::new("lljit", "não foi possível abrir a LLJIT", detail))?;
        lljit.define_runtime_symbols().map_err(|detail| {
            JitError::new(
                "runtime-symbols",
                "não foi possível publicar os símbolos do runtime nativo",
                detail,
            )
        })?;
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Ok(Self {
            modules: Vec::new(),
            reloadables: Vec::new(),
            poisoned: None,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            lljit,
        })
    }

    /// Versão da `LLVM-C.dll` carregada neste processo, `(major, minor, patch)`.
    ///
    /// Existe porque há duas instalações do LLVM 22.1.8 nesta máquina e só a
    /// completa serve; a DLL que o carregador acha primeiro no `PATH` é a que
    /// vale, e `dartforge-executar-ir --timings` a registra.
    pub fn llvm_version() -> (u32, u32, u32) {
        ffi::llvm_version()
    }

    /// Confere o alvo do módulo contra o da `LLJIT`.
    ///
    /// Um `target datalayout` ou `target triple` ausente é aceito (a `LLJIT`
    /// impõe o dela). Um presente e diferente é recusado com as duas strings:
    /// significaria que o IR foi emitido para outro alvo que não o do processo,
    /// e o conserto é no emissor, não aqui.
    fn check_target(&self, parsed: &ffi::ParsedModule) -> Result<(), JitError> {
        let (layout, triple) = parsed.target();
        let (jit_layout, jit_triple) = self.lljit.target();
        if !layout.is_empty() && layout != jit_layout {
            return Err(JitError {
                stage: "layout",
                message: format!(
                    "o módulo fixa target datalayout \"{layout}\" e a LLJIT deste processo usa \
                     \"{jit_layout}\""
                ),
            });
        }
        if !triple.is_empty() && triple != jit_triple {
            return Err(JitError {
                stage: "layout",
                message: format!(
                    "o módulo fixa target triple \"{triple}\" e a LLJIT deste processo usa \
                     \"{jit_triple}\""
                ),
            });
        }
        Ok(())
    }

    /// Nomes que os módulos residentes definem, para resolver entre módulos.
    fn defined_names(&self) -> Vec<&str> {
        self.modules
            .iter()
            .filter(|module| !module.removed)
            .flat_map(|module| module.signatures.iter().map(|s| s.name.as_str()))
            .chain(
                self.reloadables
                    .iter()
                    .flat_map(|module| module.entries.keys().map(String::as_str)),
            )
            .collect()
    }

    /// Analisa IR textual e o incorpora à sessão sob um rastreador próprio.
    ///
    /// Antes de entregar o módulo ao LLVM, confere:
    ///
    /// * o alvo (`layout`), contra o da `LLJIT`;
    /// * cada nome **declarado e não definido** (`símbolos`): tem de ser do
    ///   runtime ([`RUNTIME_SYMBOLS`]), da CRT listada ([`CRT_SYMBOLS`]), um
    ///   intrínseco `llvm.*`, ou definido por um módulo já residente.
    ///
    /// Um nome fora disso é recusado com o nome na mensagem, em vez de virar um
    /// erro de ligação do ORC — ou, pior, ser resolvido por acaso para algo
    /// homônimo do processo.
    ///
    /// # Erros
    /// IR inválido (`parse-ir`), alvo divergente (`layout`), externo
    /// desconhecido (`símbolos`) ou símbolo já definido na sessão
    /// (`add-module`). Em nenhum caso a sessão fica em estado parcial.
    pub fn add_ir_module(&mut self, name: &str, ir: &str) -> Result<ModuleReport, JitError> {
        let started = Instant::now();
        let phase = Instant::now();
        let parsed = ffi::parse_module(name, ir)
            .map_err(|detail| JitError::new("parse-ir", "IR inválido", detail))?;
        self.check_target(&parsed)?;
        let signatures = parsed.signatures();
        let defined = self.defined_names();
        if let Some(unknown) = parsed.declarations().into_iter().find(|reference| {
            !ffi::is_known_external(reference)
                && !defined.contains(&reference.as_str())
                && !signatures.iter().any(|s| &s.name == reference)
        }) {
            return Err(JitError {
                stage: "símbolos",
                message: format!(
                    "o módulo referencia {unknown}, que nem o runtime nem a lista de CRT definem, \
                     e nenhum módulo da sessão define"
                ),
            });
        }
        // A impressão digital do contrato é lida aqui, e não só no hot reload,
        // porque é o que permite a uma recarga futura comparar a versão nova com
        // esta — inclusive quando este módulo entrou pelo caminho simples.
        let layouts = parsed.class_layouts();
        let module = parsed.into_thread_safe();
        let parse_ir = phase.elapsed();
        let tracker = self.lljit.create_tracker();
        let phase = Instant::now();
        self.lljit
            .add_module(&tracker, module)
            .map_err(|detail| JitError::new("add-module", "a LLJIT recusou o módulo", detail))?;
        let add_module = phase.elapsed();
        self.modules.push(Module {
            name: name.to_owned(),
            tracker,
            removed: false,
            signatures,
            layouts,
        });
        Ok(ModuleReport {
            parse_ir,
            add_module,
            total: started.elapsed(),
            ir_bytes: ir.len(),
        })
    }

    /// Resolve um símbolo e devolve seu endereço no processo.
    ///
    /// Resolver materializa o módulo que define o símbolo, ou seja, dispara a
    /// geração de código. O endereço é informativo: chamá-lo exige conhecer a
    /// assinatura, e este crate só executa a entrada de assinatura conhecida,
    /// por [`JitSession::run_entry`].
    ///
    /// # Erros
    /// Símbolo ausente ou falha de compilação sob demanda viram erro na etapa
    /// `lookup`, com o diagnóstico do LLVM anexado.
    pub fn lookup(&self, symbol: &str) -> Result<u64, JitError> {
        self.lljit.lookup(symbol).map_err(|detail| {
            JitError::new(
                "lookup",
                &format!("não foi possível resolver o símbolo {symbol}"),
                detail,
            )
        })
    }

    /// Resolve [`ENTRY_SYMBOL`] e executa o programa como o `main` do AOT.
    ///
    /// O programa roda numa thread nova com [`PROGRAM_STACK_BYTES`] de pilha e
    /// runtime zerado, e imprime no stdout do processo. Pode ser chamada mais de
    /// uma vez: cada execução começa de um runtime limpo.
    ///
    /// # Erros
    /// Falha se nenhum módulo da sessão definir a entrada, se a compilação sob
    /// demanda desse módulo falhar (`lookup`) ou se a thread do programa não
    /// puder ser criada (`execute`).
    ///
    /// # Término
    /// Exceção não capturada, asserção de não nulidade e teto do heap encerram
    /// o processo com o código do AOT; um erro interno do runtime aborta. Ver o
    /// topo do crate.
    pub fn run_entry(&self) -> Result<EntryReport, JitError> {
        let started = Instant::now();
        let (lookup, exit_code, execute) =
            self.lljit.run_entry(PROGRAM_STACK_BYTES).map_err(|detail| {
                if detail.starts_with("não foi possível criar a thread")
                    || detail.starts_with("a thread do programa")
                {
                    JitError::new("execute", "a execução do programa falhou", detail)
                } else {
                    JitError::new(
                        "lookup",
                        &format!("não foi possível resolver o símbolo {ENTRY_SYMBOL}"),
                        detail,
                    )
                }
            })?;
        Ok(EntryReport {
            lookup,
            execute,
            total: started.elapsed(),
            exit_code,
        })
    }

    /// Nomes dos módulos ainda residentes, na ordem de inclusão.
    ///
    /// Inclui os módulos recarregáveis, um por identidade e não um por geração:
    /// as gerações são versões de um mesmo módulo, e contá-las aqui faria uma
    /// sessão recarregada parecer uma sessão com vários programas.
    pub fn module_names(&self) -> Vec<&str> {
        self.modules
            .iter()
            .filter(|module| !module.removed)
            .map(|module| module.name.as_str())
            .chain(self.reloadables.iter().map(|module| module.name.as_str()))
            .collect()
    }

    /// Descarrega o módulo de índice `index`, liberando seu código.
    ///
    /// # Segurança da remoção
    ///
    /// Remover invalida todos os endereços obtidos daquele módulo. **Só é
    /// seguro quando nenhuma função dele está executando**, em nenhuma thread e
    /// em nenhum quadro de pilha abaixo do chamador. O LLVM não verifica isso: o
    /// que era código vira memória liberada, e chamar um endereço guardado antes
    /// da remoção é uso de memória liberada.
    ///
    /// A assinatura é a única garantia possível em Rust: `&mut self` aqui contra
    /// `&self` em [`JitSession::run_entry`] — que só retorna depois que a thread
    /// do programa terminou — impede que uma execução em curso e uma remoção
    /// coexistam **nesta** sessão.
    ///
    /// # Erros
    /// Índice fora da faixa, módulo já removido ou falha do LLVM viram erro na
    /// etapa `resource`.
    pub fn remove_module(&mut self, index: usize) -> Result<(), JitError> {
        let module = self.modules.get_mut(index).ok_or_else(|| JitError {
            stage: "resource",
            message: format!("não há módulo de índice {index} nesta sessão"),
        })?;
        if module.removed {
            return Err(JitError {
                stage: "resource",
                message: format!("o módulo {} já foi removido", module.name),
            });
        }
        module.tracker.remove().map_err(|detail| {
            JitError::new("resource", "não foi possível descarregar o módulo", detail)
        })?;
        module.removed = true;
        Ok(())
    }
}

/// Compila e executa um IR numa sessão descartável, imprimindo no stdout.
///
/// É o caminho de `dartforge-executar-ir` (e, depois do merge do emissor, de
/// `dartforge run`). A sessão nasce e morre na chamada.
///
/// # Erros
/// Propaga as falhas de [`JitSession::new`], [`JitSession::add_ir_module`] e
/// [`JitSession::run_entry`].
pub fn run_ir(ir: &str) -> Result<JitReport, JitError> {
    let started = Instant::now();
    let phase = Instant::now();
    let mut session = JitSession::new()?;
    let session_time = phase.elapsed();
    let module = session.add_ir_module("dartforge", ir)?;
    let entry = session.run_entry()?;
    drop(session);
    Ok(JitReport {
        session: session_time,
        module,
        entry,
        total: started.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A mensagem de erro identifica a etapa e preserva o texto do LLVM.
    #[test]
    fn error_display_keeps_stage_and_detail() {
        let error = JitError::new(
            "parse-ir",
            "IR inválido",
            "expected top-level entity".into(),
        );
        assert_eq!(
            error.to_string(),
            "parse-ir: IR inválido: expected top-level entity"
        );
        let bare = JitError::new("lljit", "não foi possível abrir a LLJIT", String::new());
        assert_eq!(bare.to_string(), "lljit: não foi possível abrir a LLJIT");
    }
    /// A entrada nativa não se chama `main`, e o runtime não a define.
    #[test]
    fn entry_symbol_is_not_a_runtime_symbol() {
        assert_eq!(ENTRY_SYMBOL, "dartforge_entry");
        assert!(!RUNTIME_SYMBOLS.contains(&ENTRY_SYMBOL));
        assert!(!RUNTIME_SYMBOLS.contains(&"main"));
        // A tabela é gerada da fonte: ela cresce com o runtime, nunca à mão.
        assert!(RUNTIME_SYMBOLS.len() > 100, "{}", RUNTIME_SYMBOLS.len());
    }
}
