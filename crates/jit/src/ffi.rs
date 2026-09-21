//! Fronteira `unsafe` única do crate: a API C do LLVM (ORCv2) e os adaptadores
//! que expõem o runtime nativo ao código gerado em memória.
//!
//! # Por que `unsafe` é inevitável aqui
//!
//! Não existe compilador JIT seguro para este trabalho. Três operações são
//! inseguras por natureza e nenhuma abstração as elimina:
//!
//! 1. **A API C do LLVM.** `llvm-sys` publica as assinaturas `extern "C"` de
//!    ORCv2 tal como estão em `llvm-c/Orc.h` e `llvm-c/LLJIT.h`. Toda chamada é
//!    `unsafe`, e a propriedade dos recursos (contexto, módulo, tracker) segue o
//!    contrato daquela documentação, não o borrow checker.
//! 2. **Publicar endereços de funções Rust como símbolos.** O código JIT chama
//!    `@dartforge_print_i64` e companhia por endereço absoluto. Registrar esses
//!    endereços exige transformá-los em inteiros e entregá-los ao LLVM.
//! 3. **Chamar o código gerado.** O endereço devolvido por `LLVMOrcLLJITLookup`
//!    é um `u64`; invocá-lo exige `transmute` para um ponteiro de função.
//!
//! # Invariantes que este módulo preserva
//!
//! * Cada recurso do LLVM pertence a exatamente um valor Rust com `Drop`
//!   correspondente ([`Lljit`], [`ResourceTracker`], [`ThreadSafeModule`]). O
//!   que a API C consome é entregue por valor e nunca liberado duas vezes.
//! * Handles crus nunca atravessam a fronteira do crate: [`crate::JitSession`]
//!   expõe apenas `Result`, `Duration` e `String`.
//! * Os tipos daqui contêm ponteiros crus, portanto não são `Send` nem `Sync`.
//!   Isso não é acidental: o heap de [`crate::runtime`] é `thread_local` e o
//!   código gerado precisa executar na thread que abriu a sessão.
//! * Todo `extern "C"` exportado ao JIT usa a ABI escalar acordada com o
//!   emissor (`crates/llvm`), idêntica à do harness AOT. `u8` em lugar de `bool`
//!   evita estados inválidos atravessando a fronteira.
//!
//! Este módulo **não** é um verificador de segurança para IR arbitrário, pelo
//! mesmo motivo que `crates/native` não é: a garantia vale para IR emitido pelo
//! próprio compilador, com as assinaturas declaradas em `crates/llvm`.
//
// Único `allow` do crate. `src/lib.rs` e `src/runtime.rs` permanecem sob
// `unsafe_code = "deny"` do workspace, logo qualquer `unsafe` novo fora daqui
// aparece como erro de compilação, não como detalhe escondido numa revisão.
#![allow(unsafe_code)]

use llvm_sys::core::{
    LLVMContextCreate, LLVMContextDispose, LLVMCreateMemoryBufferWithMemoryRangeCopy,
    LLVMDisposeMemoryBuffer, LLVMDisposeMessage,
};
use llvm_sys::error::{LLVMDisposeErrorMessage, LLVMErrorRef, LLVMGetErrorMessage};
use llvm_sys::ir_reader::LLVMParseIRInContext2;
use llvm_sys::orc2::lljit::{
    LLVMOrcCreateLLJIT, LLVMOrcDisposeLLJIT, LLVMOrcLLJITAddLLVMIRModuleWithRT,
    LLVMOrcLLJITGetMainJITDylib, LLVMOrcLLJITLookup, LLVMOrcLLJITMangleAndIntern, LLVMOrcLLJITRef,
};
use llvm_sys::orc2::{
    LLVMJITEvaluatedSymbol, LLVMJITSymbolFlags, LLVMJITSymbolGenericFlags, LLVMOrcAbsoluteSymbols,
    LLVMOrcCSymbolMapPair, LLVMOrcDisposeMaterializationUnit, LLVMOrcDisposeThreadSafeContext,
    LLVMOrcDisposeThreadSafeModule, LLVMOrcJITDylibCreateResourceTracker, LLVMOrcJITDylibDefine,
    LLVMOrcJITDylibRef, LLVMOrcReleaseResourceTracker, LLVMOrcResourceTrackerRef,
    LLVMOrcResourceTrackerRemove, LLVMOrcThreadSafeModuleRef,
};
use llvm_sys::target::{LLVM_InitializeNativeAsmPrinter, LLVM_InitializeNativeTarget};
use std::ffi::{CStr, CString, c_char};
use std::mem::ManuallyDrop;
use std::ptr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::runtime;

/// Converte um `LLVMErrorRef` em `Option<String>` consumindo o erro.
///
/// `LLVMGetErrorMessage` consome o `LLVMErrorRef` e transfere a propriedade da
/// mensagem; devolvê-la exige copiar os bytes e liberar o original.
fn take_error(error: LLVMErrorRef) -> Option<String> {
    if error.is_null() {
        return None;
    }
    // SAFETY: `error` é não nulo e ainda não foi consumido, logo
    // `LLVMGetErrorMessage` pode consumi-lo e devolver uma string C válida, cuja
    // propriedade passa para nós. `llvm-c/Error.h` exige liberá-la com
    // `LLVMDisposeErrorMessage`, e não com o `LLVMDisposeMessage` do núcleo.
    unsafe {
        let message = LLVMGetErrorMessage(error);
        let text = CStr::from_ptr(message).to_string_lossy().into_owned();
        LLVMDisposeErrorMessage(message);
        Some(text)
    }
}

/// Copia uma mensagem `char*` do LLVM e libera o original.
///
/// # Safety
/// `message` deve ser não nulo e ter vindo de uma API do LLVM que transfere a
/// propriedade da string ao chamador.
unsafe fn take_message(message: *mut c_char) -> String {
    // SAFETY: o chamador garante que o ponteiro é válido e nosso para liberar.
    unsafe {
        let text = CStr::from_ptr(message).to_string_lossy().into_owned();
        LLVMDisposeMessage(message);
        text
    }
}

/// Inicializa o alvo nativo uma única vez por processo.
///
/// `LLVMOrcCreateLLJIT` sem builder detecta o host, o que exige os registradores
/// de alvo e o `AsmPrinter` já instalados. As rotinas do LLVM não são
/// idempotentes sob concorrência, daí o [`OnceLock`], que também memoriza a
/// falha em vez de tentar registrar os alvos de novo a cada sessão.
///
/// # Erros
/// Falha quando o LLVM ligado não inclui o backend da arquitetura do host.
pub(crate) fn initialize_native_target() -> Result<(), String> {
    static STATE: OnceLock<Result<(), String>> = OnceLock::new();
    STATE
        .get_or_init(|| {
            // SAFETY: `get_or_init` garante execução única e exclusiva; as
            // rotinas apenas registram alvos em tabelas globais do LLVM e não
            // recebem nenhum ponteiro nosso.
            let failed = unsafe {
                LLVM_InitializeNativeTarget() != 0 || LLVM_InitializeNativeAsmPrinter() != 0
            };
            if failed {
                Err("LLVM não tem backend nativo para esta arquitetura".to_owned())
            } else {
                Ok(())
            }
        })
        .clone()
}

/// Módulo LLVM já analisado e pronto para entrar numa `JITDylib`.
///
/// O contexto do LLVM que hospeda o módulo vive dentro do `ThreadSafeContext`
/// criado durante a análise; não há contexto compartilhado entre módulos.
pub(crate) struct ThreadSafeModule {
    handle: LLVMOrcThreadSafeModuleRef,
}
impl Drop for ThreadSafeModule {
    /// Libera o módulo quando ele não chegou a ser entregue à `LLJIT`.
    fn drop(&mut self) {
        // SAFETY: `handle` foi criado por `LLVMOrcCreateNewThreadSafeModule`,
        // ainda não foi consumido (o consumo usa `ManuallyDrop`) e é liberado
        // uma única vez, pois `ThreadSafeModule` não é `Copy` nem `Clone`.
        unsafe { LLVMOrcDisposeThreadSafeModule(self.handle) }
    }
}

/// Analisa IR textual e devolve um módulo pronto para execução.
///
/// Usa `LLVMParseIRInContext2`, que não consome o `MemoryBuffer`; a cópia do
/// texto pertence ao buffer e é liberada antes do retorno em qualquer caminho.
///
/// # Erros
/// Devolve o diagnóstico do próprio LLVM quando o IR é inválido, e recusa nomes
/// ou textos com byte nulo, que não sobrevivem à fronteira C.
pub(crate) fn parse_ir(name: &str, ir: &str) -> Result<ThreadSafeModule, String> {
    use llvm_sys::orc2::{
        LLVMOrcCreateNewThreadSafeContextFromLLVMContext, LLVMOrcCreateNewThreadSafeModule,
    };
    let name = CString::new(name).map_err(|_| "nome do módulo contém byte nulo".to_owned())?;
    if ir.as_bytes().contains(&0) {
        return Err("o IR contém byte nulo e não pode ser analisado".to_owned());
    }
    // SAFETY: o bloco inteiro mantém a propriedade de cada recurso explícita.
    // `buffer` é uma cópia do texto e é liberado em todos os caminhos;
    // `context` é entregue ao `ThreadSafeContext`, que é liberado logo após
    // `LLVMOrcCreateNewThreadSafeModule` reter sua própria referência; `module`
    // só é consumido por essa mesma chamada, e apenas quando a análise venceu.
    unsafe {
        let context = LLVMContextCreate();
        let buffer = LLVMCreateMemoryBufferWithMemoryRangeCopy(
            ir.as_ptr().cast::<c_char>(),
            ir.len(),
            name.as_ptr(),
        );
        let mut module = ptr::null_mut();
        let mut message = ptr::null_mut();
        let failed = LLVMParseIRInContext2(context, buffer, &mut module, &mut message) != 0;
        LLVMDisposeMemoryBuffer(buffer);
        if failed {
            let detail = if message.is_null() {
                "IR inválido".to_owned()
            } else {
                take_message(message)
            };
            LLVMContextDispose(context);
            return Err(detail);
        }
        if !message.is_null() {
            // Diagnóstico não fatal; o módulo é válido e a mensagem é liberada.
            drop(take_message(message));
        }
        let thread_safe_context = LLVMOrcCreateNewThreadSafeContextFromLLVMContext(context);
        let handle = LLVMOrcCreateNewThreadSafeModule(module, thread_safe_context);
        LLVMOrcDisposeThreadSafeContext(thread_safe_context);
        Ok(ThreadSafeModule { handle })
    }
}

/// Rastreador de recursos de um único módulo adicionado à sessão.
///
/// # Remoção
/// [`ResourceTracker::remove`] descarrega o código do módulo e invalida os
/// endereços já resolvidos a partir dele. **Só é seguro remover quando nenhuma
/// função daquele módulo está executando, direta ou indiretamente**, inclusive
/// em outra thread ou num quadro de pilha mais abaixo do chamador. O LLVM não
/// verifica isso: chamar um endereço obtido antes da remoção é uso de memória
/// liberada. Esta é a razão de [`crate::JitSession::remove_module`] exigir
/// `&mut self` — a execução do código gerado toma `&self` e as duas coisas não
/// podem coexistir.
pub(crate) struct ResourceTracker {
    handle: LLVMOrcResourceTrackerRef,
}
impl ResourceTracker {
    /// Descarrega os recursos do módulo associado a este rastreador.
    ///
    /// # Erros
    /// Propaga a falha do LLVM se a desmaterialização não puder ser concluída.
    pub(crate) fn remove(&self) -> Result<(), String> {
        // SAFETY: `handle` é um tracker vivo desta sessão; `Remove` é idempotente
        // do ponto de vista do LLVM e devolve o erro em vez de abortar.
        match take_error(unsafe { LLVMOrcResourceTrackerRemove(self.handle) }) {
            Some(message) => Err(message),
            None => Ok(()),
        }
    }
}
impl Drop for ResourceTracker {
    /// Devolve a referência ao tracker; a `LLJIT` ainda precisa estar viva.
    fn drop(&mut self) {
        // SAFETY: `crate::JitSession` declara os módulos antes da `LLJIT`, então
        // os trackers são liberados enquanto a `LLJIT` que os criou existe.
        unsafe { LLVMOrcReleaseResourceTracker(self.handle) }
    }
}

/// Marca um símbolo absoluto como visível fora do módulo e chamável.
///
/// `Exported` o torna alcançável por quem procurar o nome na dylib; `Callable`
/// informa ao ORC que o endereço é código, não um dado — distinção que importa
/// para os stubs indiretos que o hot reload usará numa etapa futura.
fn exported_callable() -> LLVMJITSymbolFlags {
    LLVMJITSymbolFlags {
        GenericFlags: LLVMJITSymbolGenericFlags::LLVMJITSymbolGenericFlagsExported as u8
            | LLVMJITSymbolGenericFlags::LLVMJITSymbolGenericFlagsCallable as u8,
        TargetFlags: 0,
    }
}

/// Instância viva da `LLJIT` e sua `JITDylib` principal.
pub(crate) struct Lljit {
    handle: LLVMOrcLLJITRef,
    main: LLVMOrcJITDylibRef,
}

impl Lljit {
    /// Cria uma `LLJIT` com o alvo do host e resolve sua `JITDylib` principal.
    ///
    /// # Erros
    /// Falha quando o alvo nativo não está disponível ou a construção da
    /// `LLJIT` retorna erro, sempre com a mensagem original do LLVM anexada.
    pub(crate) fn new() -> Result<Self, String> {
        initialize_native_target()?;
        let mut handle: LLVMOrcLLJITRef = ptr::null_mut();
        // SAFETY: `handle` é um destino válido e o builder nulo pede a
        // configuração padrão, que detecta o host já inicializado acima.
        if let Some(message) =
            take_error(unsafe { LLVMOrcCreateLLJIT(&mut handle, ptr::null_mut()) })
        {
            return Err(message);
        }
        if handle.is_null() {
            return Err("LLVM devolveu uma LLJIT nula sem diagnóstico".to_owned());
        }
        // SAFETY: `handle` acabou de ser criado com sucesso; a dylib principal
        // pertence à `LLJIT` e é válida enquanto esta estrutura viver.
        let main = unsafe { LLVMOrcLLJITGetMainJITDylib(handle) };
        Ok(Self { handle, main })
    }

    /// Publica os endereços das funções de runtime como símbolos absolutos.
    ///
    /// Esta é a contrapartida JIT da ligação que `crates/native` faz com o
    /// harness Rust: em vez de um linker resolver `@dartforge_print_i64` num
    /// objeto, a `JITDylib` passa a conter esse nome apontando para a função
    /// Rust já carregada neste processo.
    ///
    /// # Erros
    /// Falha se algum nome já estiver definido na dylib, caso em que a unidade
    /// de materialização é liberada sem alterar a sessão.
    pub(crate) fn define_runtime_symbols(&self) -> Result<(), String> {
        let mut pairs = Vec::with_capacity(RUNTIME_SYMBOLS.len());
        for (name, address) in runtime_symbol_addresses() {
            // SAFETY: `name` é um literal C estático e `self.handle` é válido.
            // A entrada devolvida já vem retida e sua propriedade é transferida
            // para `LLVMOrcAbsoluteSymbols` abaixo, que a consome.
            let interned = unsafe { LLVMOrcLLJITMangleAndIntern(self.handle, name.as_ptr()) };
            pairs.push(LLVMOrcCSymbolMapPair {
                Name: interned,
                Sym: LLVMJITEvaluatedSymbol {
                    Address: address as u64,
                    Flags: exported_callable(),
                },
            });
        }
        // SAFETY: `pairs` é um vetor contíguo de pares válidos, cujo tamanho é
        // informado exatamente. `LLVMOrcAbsoluteSymbols` consome as entradas do
        // pool de nomes, então `pairs` não pode ser reutilizado depois daqui.
        let unit = unsafe { LLVMOrcAbsoluteSymbols(pairs.as_mut_ptr(), pairs.len()) };
        // SAFETY: `unit` acabou de ser criada e é consumida por `Define` em caso
        // de sucesso; no caso de erro a propriedade volta para nós e a unidade é
        // liberada explicitamente, conforme documentado em `llvm-c/Orc.h`.
        match take_error(unsafe { LLVMOrcJITDylibDefine(self.main, unit) }) {
            None => Ok(()),
            Some(message) => {
                // SAFETY: `Define` falhou, logo `unit` continua sob nossa posse.
                unsafe { LLVMOrcDisposeMaterializationUnit(unit) };
                Err(message)
            }
        }
    }

    /// Cria um rastreador novo para os recursos de um módulo.
    pub(crate) fn create_tracker(&self) -> ResourceTracker {
        // SAFETY: `self.main` pertence a esta `LLJIT` viva; o tracker devolvido
        // já vem retido e é liberado por `ResourceTracker::drop`.
        let handle = unsafe { LLVMOrcJITDylibCreateResourceTracker(self.main) };
        ResourceTracker { handle }
    }

    /// Entrega o módulo à `LLJIT` sob o rastreador informado.
    ///
    /// # Erros
    /// Propaga a mensagem do LLVM, tipicamente definição duplicada de símbolo.
    /// Em qualquer caso o módulo foi consumido e não pode ser reaproveitado.
    pub(crate) fn add_module(
        &self,
        tracker: &ResourceTracker,
        module: ThreadSafeModule,
    ) -> Result<(), String> {
        // O LLVM assume a propriedade do módulo inclusive quando devolve erro,
        // por isso `ManuallyDrop` suprime o `Drop` local antes da chamada.
        let module = ManuallyDrop::new(module);
        // SAFETY: `module.handle` é válido e não foi consumido antes; a partir
        // desta chamada ele pertence à `LLJIT` e não é mais liberado por nós.
        match take_error(unsafe {
            LLVMOrcLLJITAddLLVMIRModuleWithRT(self.handle, tracker.handle, module.handle)
        }) {
            Some(message) => Err(message),
            None => Ok(()),
        }
    }

    /// Resolve um símbolo, materializando o código necessário.
    ///
    /// O nome é informado sem o prefixo global da plataforma: `LLJITLookup`
    /// aplica a mesma decoração usada na definição dos símbolos absolutos.
    ///
    /// # Erros
    /// Falha quando o símbolo não existe, quando a compilação sob demanda do
    /// módulo que o define falha, ou quando o nome contém byte nulo.
    pub(crate) fn lookup(&self, symbol: &str) -> Result<u64, String> {
        let symbol =
            CString::new(symbol).map_err(|_| "nome de símbolo contém byte nulo".to_owned())?;
        let mut address = 0u64;
        // SAFETY: `address` é um destino válido e `symbol` está vivo durante a
        // chamada. Materializar pode executar código do LLVM, não do programa.
        match take_error(unsafe { LLVMOrcLLJITLookup(self.handle, &mut address, symbol.as_ptr()) })
        {
            Some(message) => Err(message),
            None if address == 0 => Err("símbolo resolvido para o endereço zero".to_owned()),
            None => Ok(address),
        }
    }

    /// Resolve e executa a entrada do módulo, medindo as duas fases.
    ///
    /// A assinatura é segura de propósito: o nome do símbolo **não** vem do
    /// chamador. Resolver [`crate::ENTRY_SYMBOL`] nesta `LLJIT` viva e chamá-lo
    /// como `void(void)` é correto porque `crates/llvm` emite `@dartforge_entry`
    /// exatamente com essa assinatura, e `crates/jit` só aceita IR desse
    /// emissor — a mesma hipótese que `crates/native` faz para o AOT. Se o
    /// chamador pudesse escolher o nome, a função precisaria ser `unsafe`.
    ///
    /// Devolve `(lookup, execute)`; `lookup` inclui a geração de código sob
    /// demanda do módulo que define a entrada.
    ///
    /// # Erros
    /// Falha quando nenhum módulo da sessão define a entrada ou quando a
    /// compilação sob demanda desse módulo falha.
    pub(crate) fn run_entry(&self) -> Result<(Duration, Duration), String> {
        let phase = Instant::now();
        let address = self.lookup(crate::ENTRY_SYMBOL)?;
        let lookup = phase.elapsed();
        let phase = Instant::now();
        // SAFETY: `address` acabou de ser resolvido por esta `LLJIT`, que
        // continua viva por `&self`; o módulo que o define não pôde ser
        // removido no intervalo, porque remover exige `&mut JitSession`. A
        // conversão de inteiro para ponteiro de função é a única forma de
        // invocar código gerado em tempo de execução, e `extern "C"` é a ABI
        // declarada no IR para `@dartforge_entry`.
        unsafe {
            let entry = std::mem::transmute::<*const (), extern "C" fn()>(
                usize::try_from(address).unwrap_or(usize::MAX) as *const (),
            );
            entry();
        }
        Ok((lookup, phase.elapsed()))
    }
}

impl Drop for Lljit {
    /// Encerra a sessão do LLVM, descarregando todo o código ainda residente.
    fn drop(&mut self) {
        // SAFETY: todos os trackers desta sessão já foram liberados, porque
        // `crate::JitSession` declara o vetor de módulos antes deste campo e os
        // campos são destruídos na ordem de declaração.
        let _ = take_error(unsafe { LLVMOrcDisposeLLJIT(self.handle) });
    }
}

/// Nomes de runtime que o código JIT pode referenciar.
///
/// É o mesmo conjunto que o harness AOT define com `#[unsafe(no_mangle)]` em
/// `crates/runtime/src/runtime_main.rs`, e que `crates/llvm` declara no IR.
pub(crate) const RUNTIME_SYMBOLS: &[&str] = &[
    "dartforge_print_i64",
    "dartforge_print_bool",
    "dartforge_print_null",
    "dartforge_print_string",
    "dartforge_null_assert_fail",
    "dartforge_gc_push_frame",
    "dartforge_gc_set_root",
    "dartforge_gc_root",
    "dartforge_gc_pop_frame",
    "dartforge_gc_collect",
    "dartforge_object_new",
    "dartforge_object_get",
    "dartforge_object_set",
    "dartforge_object_class",
    "dartforge_string_new",
    "dartforge_string_concat",
    "dartforge_string_equal",
    "dartforge_enum_get",
];

/// Emparelha cada nome de runtime com o endereço da função Rust correspondente.
///
/// Tomar o endereço de uma função é seguro; o que exige cuidado é a ABI, e cada
/// adaptador abaixo repete exatamente a assinatura declarada no IR.
fn runtime_symbol_addresses() -> Vec<(&'static CStr, usize)> {
    vec![
        (c"dartforge_print_i64", print_i64 as *const () as usize),
        (c"dartforge_print_bool", print_bool as *const () as usize),
        (c"dartforge_print_null", print_null as *const () as usize),
        (
            c"dartforge_print_string",
            print_string as *const () as usize,
        ),
        (
            c"dartforge_null_assert_fail",
            null_assert_fail as *const () as usize,
        ),
        (
            c"dartforge_gc_push_frame",
            gc_push_frame as *const () as usize,
        ),
        (c"dartforge_gc_set_root", gc_set_root as *const () as usize),
        (c"dartforge_gc_root", gc_root as *const () as usize),
        (
            c"dartforge_gc_pop_frame",
            gc_pop_frame as *const () as usize,
        ),
        (c"dartforge_gc_collect", gc_collect as *const () as usize),
        (c"dartforge_object_new", object_new as *const () as usize),
        (c"dartforge_object_get", object_get as *const () as usize),
        (c"dartforge_object_set", object_set as *const () as usize),
        (
            c"dartforge_object_class",
            object_class as *const () as usize,
        ),
        (c"dartforge_string_new", string_new as *const () as usize),
        (
            c"dartforge_string_concat",
            string_concat as *const () as usize,
        ),
        (
            c"dartforge_string_equal",
            string_equal as *const () as usize,
        ),
        (c"dartforge_enum_get", enum_get as *const () as usize),
    ]
}

// Adaptadores `extern "C"` chamados pelo código gerado. Todos delegam a
// `crate::runtime`, que é Rust seguro. Como são `extern "C"` (e não
// `extern "C-unwind"`), um `panic` do runtime aborta o processo em vez de
// desenrolar por quadros de pilha gerados pelo LLVM — que não sabem desenrolar.
// Isso é deliberado: o contrato de `crates/runtime` já usa `panic` para erro
// interno de compilador, e abortar é o final definido para essa situação.

/// Imprime um inteiro assinado; contrato `void(i64)`.
extern "C" fn print_i64(value: i64) {
    runtime::print_i64(value);
}
/// Imprime um booleano codificado em byte; contrato `void(i8)`.
extern "C" fn print_bool(value: u8) {
    runtime::print_bool(value);
}
/// Imprime `null`; contrato `void(void)`.
extern "C" fn print_null() {
    runtime::print_null();
}
/// Imprime uma string gerenciada por handle; contrato `void(i64)`.
extern "C" fn print_string(handle: i64) {
    runtime::print_string(handle);
}
/// Encerra o processo numa asserção de não nulidade; contrato `void(void) noreturn`.
extern "C" fn null_assert_fail() -> ! {
    runtime::null_assert_fail()
}
/// Abre um frame de raízes; contrato `i64(i64)`.
extern "C" fn gc_push_frame(slot_count: i64) -> i64 {
    runtime::gc_push_frame(slot_count)
}
/// Grava uma raiz estática do frame; contrato `void(i64, i64, i64)`.
extern "C" fn gc_set_root(frame: i64, slot: i64, handle: i64) {
    runtime::gc_set_root(frame, slot, handle);
}
/// Acrescenta uma raiz dinâmica ao frame; contrato `void(i64, i64)`.
extern "C" fn gc_root(frame: i64, handle: i64) {
    runtime::gc_root(frame, handle);
}
/// Encerra um frame de raízes; contrato `void(i64)`.
extern "C" fn gc_pop_frame(frame: i64) {
    runtime::gc_pop_frame(frame);
}
/// Dispara uma coleta explícita; contrato `void(void)`.
extern "C" fn gc_collect() {
    runtime::gc_collect();
}
/// Aloca um objeto zerado; contrato `i64(i64, i64)`.
extern "C" fn object_new(class_id: i64, field_count: i64) -> i64 {
    runtime::object_new(class_id, field_count)
}
/// Lê os bits de um campo; contrato `i64(i64, i64)`.
extern "C" fn object_get(handle: i64, index: i64) -> i64 {
    runtime::object_get(handle, index)
}
/// Grava um campo com tag explícita de referência; contrato `void(i64, i64, i64, i8)`.
extern "C" fn object_set(handle: i64, index: i64, bits: i64, is_ref: u8) {
    runtime::object_set(handle, index, bits, is_ref);
}
/// Consulta a classe nominal de um objeto; contrato `i64(i64)`.
extern "C" fn object_class(handle: i64) -> i64 {
    runtime::object_class(handle)
}
/// Concatena duas strings gerenciadas; contrato `i64(i64, i64)`.
extern "C" fn string_concat(a: i64, b: i64) -> i64 {
    runtime::string_concat(a, b)
}
/// Compara duas strings gerenciadas; contrato `i8(i64, i64)`.
extern "C" fn string_equal(a: i64, b: i64) -> u8 {
    runtime::string_equal(a, b)
}

/// Cria uma string gerenciada a partir de uma constante do módulo.
///
/// Contrato `i64(ptr, i64)`.
///
/// # Safety
/// `ptr` precisa endereçar `len` bytes legíveis e imutáveis durante a chamada.
/// O emissor garante isso: o argumento é sempre uma constante global do próprio
/// módulo, viva enquanto o módulo estiver carregado.
unsafe extern "C" fn string_new(ptr: *const u8, len: i64) -> i64 {
    // SAFETY: o contrato acima é cumprido pelo IR emitido por `crates/llvm`; a
    // fatia não sobrevive à chamada e os bytes não são modificados.
    runtime::string_new(unsafe { borrow_bytes(ptr, len) })
}

/// Obtém um valor de enum canônico pelo nome constante.
///
/// Contrato `i64(i64, i64, ptr, i64)`.
///
/// # Safety
/// Mesmas exigências de [`string_new`] para `ptr`/`len`.
unsafe extern "C" fn enum_get(class_id: i64, index: i64, ptr: *const u8, len: i64) -> i64 {
    // SAFETY: idem `string_new`; o nome é constante global do próprio módulo.
    runtime::enum_get(class_id, index, unsafe { borrow_bytes(ptr, len) })
}

/// Empresta os bytes de uma constante do módulo como fatia.
///
/// # Safety
/// `ptr` precisa endereçar `len` bytes legíveis, ou `len` precisa ser zero. Um
/// `len` negativo é rejeitado com `panic`, porque indica IR corrompido e não um
/// erro recuperável do programa Dart.
unsafe fn borrow_bytes<'a>(ptr: *const u8, len: i64) -> &'a [u8] {
    let len = usize::try_from(len).expect("comprimento inválido");
    if len == 0 {
        return &[];
    }
    // SAFETY: o chamador garante `len` bytes legíveis a partir de `ptr`; a
    // fatia devolvida não escapa além da chamada do adaptador que a criou.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A tabela de endereços cobre exatamente os nomes anunciados pelo crate.
    #[test]
    fn symbol_table_matches_the_announced_names() {
        let addresses = runtime_symbol_addresses();
        assert_eq!(addresses.len(), RUNTIME_SYMBOLS.len());
        for (index, (name, address)) in addresses.iter().enumerate() {
            assert_eq!(name.to_str().unwrap(), RUNTIME_SYMBOLS[index]);
            assert_ne!(*address, 0);
        }
    }
}
