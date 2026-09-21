//! Execução em memória do subconjunto nativo, via LLVM ORCv2 (`LLJIT`).
//!
//! Este é o **perfil de desenvolvimento** do DartForge. O perfil de produção
//! continua sendo o AOT de `crates/native`: IR → objeto por Clang → executável
//! ligado ao runtime Rust. Os dois consomem exatamente o mesmo LLVM IR, emitido
//! por `dartforge_compiler::compile_llvm_with_options`, e o teste diferencial em
//! `tests/execucao.rs` existe para provar que produzem a mesma saída.
//!
//! A diferença está no que acontece depois do IR:
//!
//! | | AOT (`crates/native`) | JIT (este crate) |
//! | --- | --- | --- |
//! | Geração de código | Clang, em processo separado | ORCv2, em memória |
//! | Runtime | `rustc` compila `RUNTIME_MAIN` e o linker resolve | endereços das funções Rust registrados como símbolos absolutos |
//! | Saída | executável em disco | nada em disco |
//! | Isolamento | processo próprio | **mesmo processo** do compilador |
//!
//! A última linha é o limite mais importante: o código gerado executa dentro do
//! processo hospedeiro, com o mesmo heap gerenciado e o mesmo stdout. Um erro
//! interno do runtime aborta o hospedeiro. [`docs/JIT.md`] detalha esses
//! limites.
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
//! let (saida, relatorio) = dartforge_jit::run_ir_capturing(ir)?;
//! assert_eq!(saida, "7\n");
//! assert!(relatorio.total >= relatorio.entry.execute);
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
mod runtime;

use std::time::{Duration, Instant};

/// Nome do símbolo que `crates/llvm` emite para o corpo de `main`.
///
/// O emissor nativo não produz uma função chamada `main`: o corpo de `main` e as
/// instruções de topo do módulo viram `void @dartforge_entry()`. Quem procurar
/// `main` por engano recebe erro de símbolo ausente, não uma chamada errada.
pub const ENTRY_SYMBOL: &str = "dartforge_entry";

/// Nomes do runtime nativo que a sessão define para o código gerado.
///
/// É o mesmo conjunto que o harness AOT exporta com `#[unsafe(no_mangle)]`.
///
/// ```
/// assert!(dartforge_jit::RUNTIME_SYMBOLS.contains(&"dartforge_print_i64"));
/// assert!(!dartforge_jit::RUNTIME_SYMBOLS.contains(&"dartforge_entry"));
/// ```
pub const RUNTIME_SYMBOLS: &[&str] = ffi::RUNTIME_SYMBOLS;

/// Etapa e diagnóstico de uma falha da sessão JIT.
///
/// `message` traz a explicação em português e, quando existe, o texto original
/// do LLVM anexado. Nenhum caminho de erro esperado usa `panic`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JitError {
    /// Etapa que falhou: `lljit`, `runtime-symbols`, `parse-ir`, `add-module`,
    /// `lookup` ou `resource`.
    pub stage: &'static str,
    /// Explicação em português, com o diagnóstico do LLVM quando houver.
    pub message: String,
}
impl std::fmt::Display for JitError {
    /// Apresenta a etapa antes da mensagem, como faz `dartforge_native`.
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
/// mesmo espírito de `LinkStats` (veja `docs/DESEMPENHO.md`). `total` cobre a
/// chamada inteira e é pelo menos a soma das fases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleReport {
    /// Análise do IR textual e construção do `ThreadSafeModule`.
    pub parse_ir: Duration,
    /// Entrega do módulo à `LLJIT` sob um `ResourceTracker` próprio.
    pub add_module: Duration,
    /// Chamada completa de [`JitSession::add_ir_module`].
    pub total: Duration,
    /// Bytes de IR textual analisados, para separar tempo de tamanho.
    pub ir_bytes: usize,
}

/// Custo de resolver e executar um símbolo da sessão.
///
/// `lookup` inclui a **geração de código sob demanda**: no ORCv2 o módulo só é
/// compilado quando algum de seus símbolos é procurado. Por isso o primeiro
/// `lookup` de uma sessão costuma dominar o relatório, e o `execute` seguinte
/// mede apenas a execução do código já materializado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryReport {
    /// Resolução do símbolo, incluindo a compilação sob demanda do módulo.
    pub lookup: Duration,
    /// Execução do código gerado, do `call` ao retorno.
    pub execute: Duration,
    /// Chamada completa de [`JitSession::run_entry`].
    pub total: Duration,
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
/// A sessão não é `Send` nem `Sync`. O heap gerenciado é `thread_local`, então o
/// código gerado precisa executar na thread que criou a sessão; em outra thread
/// ele veria um heap vazio e os handles não fariam sentido.
///
/// # Símbolos duplicados
///
/// Todos os módulos entram na mesma `JITDylib`, onde cada nome é definido uma
/// única vez. Como todo programa Dart compilado para o alvo nativo define
/// `@dartforge_entry`, **dois programas completos não cabem na mesma sessão**:
/// o segundo `add_ir_module` devolve erro de definição duplicada. Use uma sessão
/// por programa, ou módulos com símbolos distintos.
pub struct JitSession {
    /// Rastreadores dos módulos, na ordem em que foram adicionados.
    ///
    /// Declarado **antes** de `lljit` de propósito: os campos são destruídos na
    /// ordem de declaração, e liberar um `ResourceTracker` depois de destruir a
    /// `LLJIT` que o criou seria uso de memória liberada.
    modules: Vec<Module>,
    lljit: ffi::Lljit,
}

/// Um módulo já incorporado à sessão e o rastreador que o descarrega.
struct Module {
    name: String,
    tracker: ffi::ResourceTracker,
    removed: bool,
}

impl JitSession {
    /// Abre uma `LLJIT` para o host e publica os símbolos do runtime nativo.
    ///
    /// Os símbolos vêm de [`RUNTIME_SYMBOLS`] e são registrados como endereços
    /// absolutos das funções Rust deste processo. Foi a alternativa escolhida em
    /// vez do gerador de símbolos do processo
    /// (`LLVMOrcCreateDynamicLibrarySearchGeneratorForProcess`) por três razões:
    /// o runtime do DartForge **não é** exportado pelo executável hospedeiro —
    /// `crates/runtime` publica o harness como texto Rust, compilado pelo driver
    /// AOT, não como biblioteca com símbolos exportados; o gerador de processo
    /// exporia ao código gerado todo símbolo do processo, uma superfície muito
    /// maior que o contrato de `crates/llvm`; e a lista explícita falha alto
    /// quando o emissor passa a declarar um nome novo, em vez de resolvê-lo por
    /// acidente para algo homônimo.
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
        Ok(Self {
            modules: Vec::new(),
            lljit,
        })
    }

    /// Analisa IR textual e o incorpora à sessão sob um rastreador próprio.
    ///
    /// `name` identifica o módulo nos relatórios e nos diagnósticos do LLVM; não
    /// precisa ser único, mas ajuda quando é. O IR não pode fixar `target
    /// triple` nem `target datalayout`: a `LLJIT` impõe o layout do host, e um
    /// layout divergente é recusado pelo próprio LLVM.
    ///
    /// # Erros
    /// IR inválido devolve o diagnóstico do LLVM na etapa `parse-ir`; símbolo já
    /// definido na sessão devolve erro na etapa `add-module`. Em nenhum dos dois
    /// casos a sessão fica em estado parcial: o módulo não é registrado.
    pub fn add_ir_module(&mut self, name: &str, ir: &str) -> Result<ModuleReport, JitError> {
        let started = Instant::now();
        let phase = Instant::now();
        let module = ffi::parse_ir(name, ir)
            .map_err(|detail| JitError::new("parse-ir", "IR inválido", detail))?;
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

    /// Resolve e executa [`ENTRY_SYMBOL`], imprimindo no stdout do processo.
    ///
    /// # Erros
    /// Falha se nenhum módulo da sessão definir a entrada, ou se a compilação
    /// sob demanda desse módulo falhar.
    ///
    /// # Panics
    /// Não entra em `panic` por erro esperado. Um erro **interno** do runtime
    /// (handle inválido, tipo errado num campo) chega como `panic` numa função
    /// `extern "C"`, o que aborta o processo; e uma asserção de não nulidade
    /// falha encerra o processo com código 101, como no AOT.
    pub fn run_entry(&self) -> Result<EntryReport, JitError> {
        let started = Instant::now();
        let (lookup, execute) = self.lljit.run_entry().map_err(|detail| {
            JitError::new(
                "lookup",
                &format!("não foi possível resolver o símbolo {ENTRY_SYMBOL}"),
                detail,
            )
        })?;
        Ok(EntryReport {
            lookup,
            execute,
            total: started.elapsed(),
        })
    }

    /// Executa a entrada capturando as linhas impressas em vez de escrevê-las.
    ///
    /// As linhas usam `\n` em qualquer sistema. A captura vale apenas para as
    /// funções de impressão do runtime; qualquer escrita direta em stdout feita
    /// por outra parte do processo continua indo para o stdout real.
    ///
    /// # Erros
    /// Os mesmos de [`JitSession::run_entry`]; em caso de erro o texto
    /// eventualmente já impresso é descartado junto com o resultado.
    pub fn run_entry_capturing(&self) -> Result<(String, EntryReport), JitError> {
        let (report, output) = runtime::capturing(|| self.run_entry());
        report.map(|report| (output, report))
    }

    /// Nomes dos módulos ainda residentes, na ordem de inclusão.
    pub fn module_names(&self) -> Vec<&str> {
        self.modules
            .iter()
            .filter(|module| !module.removed)
            .map(|module| module.name.as_str())
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
    /// `&self` em [`JitSession::run_entry`] impede que uma execução em curso e
    /// uma remoção coexistam **nesta** sessão. O que o compilador não pode
    /// impedir é a remoção a partir de dentro de uma chamada ao código gerado —
    /// por exemplo, de dentro de uma função de runtime invocada pelo programa. O
    /// hot reload, que virá numa etapa seguinte, depende de stubs indiretos
    /// justamente para não precisar dessa garantia manual.
    ///
    /// Os handles do heap gerenciado **não** são liberados: o heap pertence à
    /// thread, não ao módulo, e sobrevive à remoção.
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

    /// Estatísticas do heap gerenciado da thread, equivalentes a `DARTFORGE_GC_STATS`.
    ///
    /// O heap é da thread e acumula entre execuções da mesma sessão; não é
    /// reiniciado ao adicionar ou remover módulos.
    pub fn gc_stats(&self) -> dartforge_runtime::heap::HeapStats {
        runtime::gc_stats()
    }
}

/// Compila e executa um IR numa sessão descartável, imprimindo no stdout.
///
/// É o caminho usado por `dartforge run`. A sessão nasce e morre na chamada, o
/// que significa que todo o código gerado é descartado ao final — apropriado
/// para uma execução única, não para um laço de desenvolvimento, que deve
/// manter uma [`JitSession`] aberta.
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

/// Igual a [`run_ir`], mas devolve as linhas impressas em vez de escrevê-las.
///
/// # Erros
/// As mesmas de [`run_ir`].
pub fn run_ir_capturing(ir: &str) -> Result<(String, JitReport), JitError> {
    let (report, output) = runtime::capturing(|| run_ir(ir));
    report.map(|report| (output, report))
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
        assert_eq!(RUNTIME_SYMBOLS.len(), 18);
    }
}
