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
//!   O estado do runtime é `thread_local`; o programa executa numa thread
//!   própria por execução ([`execute_program`]).
//! * O runtime publicado ao JIT **não é reescrito aqui**: é a fonte do harness
//!   AOT (os fragmentos de `crates/runtime/src`), compilada como o módulo
//!   `dartforge_runtime::abi`; a tabela de símbolos é gerada da mesma fonte
//!   pelo `build.rs` daquele crate (fonte única).
//!
//! Este módulo **não** é um verificador de segurança para IR arbitrário, pelo
//! mesmo motivo que o driver AOT não é: a garantia vale para IR emitido pelo
//! próprio compilador (`crates/emit_native`), com as assinaturas que ele declara.
//
// Único `allow` do crate. `src/lib.rs` e `src/reload.rs` permanecem sob
// `unsafe_code = "deny"` do workspace, logo qualquer `unsafe` novo fora daqui
// aparece como erro de compilação, não como detalhe escondido numa revisão.
#![allow(unsafe_code)]

use llvm_sys::core::{
    LLVMAddFunction, LLVMConstIntGetSExtValue, LLVMContextCreate, LLVMContextDispose,
    LLVMCountParamTypes, LLVMCreateMemoryBufferWithMemoryRangeCopy, LLVMDisposeMemoryBuffer,
    LLVMDisposeMessage, LLVMDisposeModule, LLVMGetCalledValue, LLVMGetFirstBasicBlock,
    LLVMGetFirstFunction, LLVMGetFirstInstruction, LLVMGetNextBasicBlock, LLVMGetNextFunction,
    LLVMGetNextInstruction, LLVMGetNumOperands, LLVMGetOperand, LLVMGetParamTypes,
    LLVMGetReturnType, LLVMGetValueName2, LLVMGlobalGetValueType, LLVMIsACallInst,
    LLVMIsAConstantInt, LLVMIsDeclaration, LLVMIsFunctionVarArg, LLVMPrintTypeToString,
    LLVMReplaceAllUsesWith, LLVMSetValueName2,
};
use llvm_sys::error::{LLVMDisposeErrorMessage, LLVMErrorRef, LLVMGetErrorMessage};
use llvm_sys::ir_reader::LLVMParseIRInContext2;
use llvm_sys::orc2::lljit::{
    LLVMOrcCreateLLJIT, LLVMOrcCreateLLJITBuilder, LLVMOrcDisposeLLJIT,
    LLVMOrcLLJITAddLLVMIRModuleWithRT,
    LLVMOrcLLJITBuilderSetJITTargetMachineBuilder, LLVMOrcLLJITGetDataLayoutStr,
    LLVMOrcLLJITGetMainJITDylib, LLVMOrcLLJITGetTripleString, LLVMOrcLLJITLookup,
    LLVMOrcLLJITMangleAndIntern, LLVMOrcLLJITRef,
};
use llvm_sys::orc2::{
    LLVMJITEvaluatedSymbol, LLVMJITSymbolFlags, LLVMJITSymbolGenericFlags, LLVMOrcAbsoluteSymbols,
    LLVMOrcCSymbolMapPair, LLVMOrcDisposeMaterializationUnit, LLVMOrcDisposeThreadSafeContext,
    LLVMOrcDisposeThreadSafeModule, LLVMOrcJITDylibCreateResourceTracker, LLVMOrcJITDylibDefine,
    LLVMOrcJITDylibRef, LLVMOrcJITTargetMachineBuilderCreateFromTargetMachine,
    LLVMOrcReleaseResourceTracker, LLVMOrcResourceTrackerRef, LLVMOrcResourceTrackerRemove,
    LLVMOrcThreadSafeModuleRef,
};
use llvm_sys::prelude::{LLVMContextRef, LLVMModuleRef, LLVMTypeRef, LLVMValueRef};
use llvm_sys::target::{LLVM_InitializeNativeAsmPrinter, LLVM_InitializeNativeTarget};
use llvm_sys::target_machine::{
    LLVMCodeGenFileType, LLVMCodeGenOptLevel, LLVMCodeModel, LLVMCreateTargetMachine,
    LLVMDisposeTargetMachine, LLVMGetDefaultTargetTriple, LLVMGetTargetFromTriple, LLVMRelocMode,
    LLVMTargetMachineEmitToMemoryBuffer, LLVMTargetMachineRef,
};
use std::ffi::{CStr, CString, c_char};
use std::mem::ManuallyDrop;
use std::ptr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

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

/// Módulo LLVM já analisado, ainda **fora** da sessão.
///
/// Existe para dar um ponto de inspeção e reescrita entre a análise do IR e a
/// entrada na `LLJIT`: é aqui que o hot reload lê a impressão digital do
/// contrato ([`ParsedModule::signatures`], [`ParsedModule::class_layouts`]) e
/// versiona as implementações ([`ParsedModule::version_definitions`]) antes de
/// qualquer efeito sobre o código que está executando.
///
/// O contexto e o módulo pertencem a este valor até [`ParsedModule::into_thread_safe`];
/// depois disso pertencem ao `ThreadSafeModule` devolvido.
pub(crate) struct ParsedModule {
    context: LLVMContextRef,
    module: LLVMModuleRef,
}

impl Drop for ParsedModule {
    /// Libera módulo e contexto quando o módulo não chegou à `LLJIT`.
    ///
    /// A ordem importa: o módulo pertence ao contexto, logo é liberado primeiro.
    /// `into_thread_safe` usa `ManuallyDrop` para não passar por aqui.
    fn drop(&mut self) {
        // SAFETY: os dois ponteiros vêm de `parse_module`, que só constrói este
        // valor depois de uma análise bem-sucedida, e nenhum deles foi consumido
        // (o consumo em `into_thread_safe` suprime este `Drop`).
        unsafe {
            LLVMDisposeModule(self.module);
            LLVMContextDispose(self.context);
        }
    }
}

/// Assinatura de uma função do módulo, em texto de LLVM IR.
///
/// É a **impressão digital do contrato de chamada** de uma função: o que precisa
/// permanecer igual entre gerações para que a troca de implementação seja segura.
/// O hash do corpo não entra aqui de propósito — o corpo é exatamente o que o
/// hot reload troca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FunctionSignature {
    /// Nome do símbolo tal como o emissor o produziu (`df_fn_0`, `dartforge_entry`).
    pub(crate) name: String,
    /// Tipo de retorno em texto de IR (`i64`, `void`).
    pub(crate) ret: String,
    /// Tipos dos parâmetros em texto de IR, na ordem da chamada.
    pub(crate) params: Vec<String>,
    /// Função variádica; fora do contrato do emissor nativo.
    pub(crate) var_arg: bool,
}

impl FunctionSignature {
    /// Apresenta a assinatura como o LLVM a escreveria: `i64 (i64, i64)`.
    pub(crate) fn text(&self) -> String {
        format!("{} ({})", self.ret, self.params.join(", "))
    }
    /// Lista de parâmetros nomeada `%a0, %a1, ...`, para gerar o trampolim.
    pub(crate) fn declaration_params(&self) -> String {
        self.params
            .iter()
            .enumerate()
            .map(|(index, ty)| format!("{ty} %a{index}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Analisa IR textual e devolve o módulo aberto para inspeção.
///
/// Usa `LLVMParseIRInContext2`, que não consome o `MemoryBuffer`; a cópia do
/// texto pertence ao buffer e é liberada antes do retorno em qualquer caminho.
///
/// # Erros
/// Devolve o diagnóstico do próprio LLVM quando o IR é inválido, e recusa nomes
/// ou textos com byte nulo, que não sobrevivem à fronteira C.
pub(crate) fn parse_module(name: &str, ir: &str) -> Result<ParsedModule, String> {
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
        // O analisador de IR textual não roda o verificador do LLVM, e o ORC
        // também não. O Clang roda, e recusa o módulo. Sem esta chamada, o JIT
        // gerava código para IR que o AOT recusa, como uma instrução que não
        // domina todos os usos, e o programa executava ou caía no JIT e não
        // compilava no AOT. O corpus inteiro no CI mostrou 13 programas assim
        // (pesado.yml, job `jit`). Aceitar o mesmo IR que o AOT aceita é parte
        // do contrato.
        let mut detalhe = ptr::null_mut();
        let invalido = llvm_sys::analysis::LLVMVerifyModule(
            module,
            llvm_sys::analysis::LLVMVerifierFailureAction::LLVMReturnStatusAction,
            &mut detalhe,
        ) != 0;
        let texto = if detalhe.is_null() { String::new() } else { take_message(detalhe) };
        if invalido {
            LLVMDisposeModule(module);
            LLVMContextDispose(context);
            let primeira = texto.lines().next().unwrap_or("módulo inválido").to_owned();
            return Err(format!("o verificador do LLVM recusou o módulo: {primeira}"));
        }
        Ok(ParsedModule { context, module })
    }
}

/// Analisa IR textual e devolve um módulo pronto para entrar numa `JITDylib`.
///
/// # Erros
/// Os mesmos de [`parse_module`].
pub(crate) fn parse_ir(name: &str, ir: &str) -> Result<ThreadSafeModule, String> {
    Ok(parse_module(name, ir)?.into_thread_safe())
}

impl ParsedModule {
    /// Empacota o módulo num `ThreadSafeModule`, transferindo a propriedade.
    pub(crate) fn into_thread_safe(self) -> ThreadSafeModule {
        use llvm_sys::orc2::{
            LLVMOrcCreateNewThreadSafeContextFromLLVMContext, LLVMOrcCreateNewThreadSafeModule,
        };
        // O `ThreadSafeContext` assume o contexto e o `ThreadSafeModule` assume o
        // módulo; nenhum dos dois pode ser liberado por `Drop` depois daqui.
        let this = ManuallyDrop::new(self);
        // SAFETY: `context` e `module` são válidos e ainda não foram consumidos.
        // `LLVMOrcCreateNewThreadSafeContextFromLLVMContext` passa a ser o dono
        // do contexto; `LLVMOrcCreateNewThreadSafeModule` retém sua própria
        // referência ao contexto seguro, então a nossa pode ser devolvida logo
        // depois, como já fazia o caminho não recarregável.
        unsafe {
            let thread_safe_context =
                LLVMOrcCreateNewThreadSafeContextFromLLVMContext(this.context);
            let handle = LLVMOrcCreateNewThreadSafeModule(this.module, thread_safe_context);
            LLVMOrcDisposeThreadSafeContext(thread_safe_context);
            ThreadSafeModule { handle }
        }
    }

    /// Assinaturas das funções **definidas** pelo módulo, na ordem do IR.
    ///
    /// Declarações externas ficam de fora: elas são o que o módulo consome (o
    /// runtime), não o que ele oferece para ser recarregado.
    pub(crate) fn signatures(&self) -> Vec<FunctionSignature> {
        self.definitions()
            .into_iter()
            // SAFETY: cada `function` é uma definição viva deste módulo,
            // devolvida pela travessia do próprio LLVM.
            .map(|function| unsafe { self.signature_of(function) })
            .collect()
    }

    /// Nomes que o módulo **declara sem definir**, isto é, o que ele consome.
    ///
    /// Numa sessão JIT esse conjunto precisa estar contido na tabela do runtime,
    /// na lista de CRT ([`CRT_SYMBOLS`]) e no que outros módulos já definem. Conferir isso antes de
    /// mexer na sessão troca um erro de ligação do ORC por um diagnóstico com o
    /// nome que falta.
    pub(crate) fn declarations(&self) -> Vec<String> {
        let mut names = Vec::new();
        // SAFETY: travessia pela API do LLVM, terminada no primeiro nulo; os
        // valores são válidos enquanto este módulo não for consumido.
        unsafe {
            let mut function = LLVMGetFirstFunction(self.module);
            while !function.is_null() {
                if LLVMIsDeclaration(function) != 0 {
                    names.push(value_name(function));
                }
                function = LLVMGetNextFunction(function);
            }
        }
        names
    }

    /// `target datalayout` e `target triple` do módulo, vazios quando ausentes.
    pub(crate) fn target(&self) -> (String, String) {
        // SAFETY: as duas funções devolvem strings que pertencem ao módulo vivo;
        // são copiadas antes de qualquer outra chamada ao LLVM.
        unsafe {
            (
                CStr::from_ptr(llvm_sys::core::LLVMGetDataLayoutStr(self.module))
                    .to_string_lossy()
                    .into_owned(),
                CStr::from_ptr(llvm_sys::core::LLVMGetTarget(self.module))
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    }

    /// Layout nominal das classes construídas pelo módulo: `(class_id, campos)`.
    ///
    /// Sai das chamadas a `@dartforge_object_new(i64 class_id, i64 field_count)`
    /// com os dois argumentos constantes, que é a forma que `crates/llvm` emite
    /// em `@df_new_*`. Duas gerações que discordem no número de campos de uma
    /// mesma classe têm layouts incompatíveis: os objetos já vivos no heap
    /// gerenciado continuariam com o layout antigo, e o código novo leria campos
    /// que não existem. Por isso essa divergência recusa a recarga em vez de
    /// corromper o heap em silêncio.
    ///
    /// Classes que o módulo novo não constrói não aparecem, e portanto não são
    /// verificadas — limite declarado em `docs/JIT.md`.
    pub(crate) fn class_layouts(&self) -> Vec<(i64, i64)> {
        let mut layouts = Vec::new();
        for function in self.definitions() {
            // SAFETY: travessia da estrutura do módulo pela própria API do LLVM.
            // Cada ponteiro devolvido é válido enquanto o módulo existir, e o
            // laço termina no primeiro nulo, como manda `llvm-c/Core.h`.
            unsafe {
                let mut block = LLVMGetFirstBasicBlock(function);
                while !block.is_null() {
                    let mut instruction = LLVMGetFirstInstruction(block);
                    while !instruction.is_null() {
                        if let Some(layout) = object_new_layout(instruction) {
                            layouts.push(layout);
                        }
                        instruction = LLVMGetNextInstruction(instruction);
                    }
                    block = LLVMGetNextBasicBlock(block);
                }
            }
        }
        layouts.sort_unstable();
        layouts.dedup();
        layouts
    }

    /// Versiona as implementações e desvia todas as chamadas para as entradas.
    ///
    /// Para cada função definida `F` o módulo passa a conter:
    ///
    /// * a **implementação** renomeada para `F$sufixo`, que é o que esta geração
    ///   materializa e o que o `ResourceTracker` desta geração descarrega;
    /// * uma **declaração externa** `F`, que é a entrada estável, resolvida pelo
    ///   módulo de trampolins da sessão.
    ///
    /// `LLVMReplaceAllUsesWith` troca *todos* os usos da implementação pela
    /// declaração, inclusive os de dentro do próprio módulo e as recursões. É
    /// isso que faz valer a regra do plano — «os chamadores passam pela entrada,
    /// nunca pelo corpo».
    ///
    /// Hoje essa indireção interna **não é observável**: `hot_reload` toma
    /// `&mut self` e executar toma `&self`, logo nenhuma recarga acontece com
    /// código gerado na pilha, e o caso «quadro antigo faz uma chamada nova» não
    /// existe. Ela está aqui para que a regra continue valendo quando a recarga
    /// for disparada de fora do fluxo — de um observador de arquivos, ou de dentro
    /// de uma função de runtime chamada pelo programa. Sem ela, esse passo
    /// exigiria reescrever call sites já materializados.
    ///
    /// Devolve as assinaturas na ordem do IR, com os nomes **estáveis** (sem o
    /// sufixo), que é a chave da tabela de entradas da sessão.
    pub(crate) fn version_definitions(&self, suffix: &str) -> Vec<FunctionSignature> {
        let mut signatures = Vec::new();
        // A lista de definições é fotografada antes da primeira reescrita: os
        // valores continuam válidos enquanto o módulo existir, e as declarações
        // acrescentadas no laço não voltam a ser visitadas.
        for function in self.definitions() {
            // SAFETY: `function` é uma definição viva deste módulo.
            let signature = unsafe { self.signature_of(function) };
            let versioned = format!("{}{suffix}", signature.name);
            let Ok(stable) = CString::new(signature.name.as_str()) else {
                continue;
            };
            // SAFETY: os dois nomes vivem até o fim da iteração. `AddFunction`
            // cria uma declaração (sem corpo) com exatamente o tipo da
            // implementação, o que mantém todas as chamadas bem tipadas depois do
            // `RAUW`. Renomear antes de declarar é obrigatório: dois valores
            // globais não podem dividir um nome, e o LLVM resolveria o conflito
            // sufixando o segundo, quebrando a entrada estável em silêncio.
            unsafe {
                let kind = LLVMGlobalGetValueType(function);
                LLVMSetValueName2(
                    function,
                    versioned.as_ptr().cast::<c_char>(),
                    versioned.len(),
                );
                let entry = LLVMAddFunction(self.module, stable.as_ptr(), kind);
                LLVMReplaceAllUsesWith(function, entry);
            }
            signatures.push(signature);
        }
        signatures
    }

    /// Lê a assinatura de uma função definida.
    ///
    /// # Safety
    /// `function` precisa ser uma função viva deste módulo.
    unsafe fn signature_of(&self, function: LLVMValueRef) -> FunctionSignature {
        // SAFETY: o chamador garante a validade; `GlobalGetValueType` de uma
        // função devolve seu tipo de função, e as consultas seguintes apenas
        // leem esse tipo.
        unsafe {
            let kind = LLVMGlobalGetValueType(function);
            FunctionSignature {
                name: value_name(function),
                ret: type_text(LLVMGetReturnType(kind)),
                params: param_types(kind)
                    .into_iter()
                    .map(|param| type_text(param))
                    .collect(),
                var_arg: LLVMIsFunctionVarArg(kind) != 0,
            }
        }
    }

    /// Funções com corpo, na ordem do IR.
    fn definitions(&self) -> Vec<LLVMValueRef> {
        let mut functions = Vec::new();
        // SAFETY: travessia pela API do LLVM, terminada no primeiro nulo; os
        // valores são válidos enquanto este módulo não for consumido.
        unsafe {
            let mut function = LLVMGetFirstFunction(self.module);
            while !function.is_null() {
                if LLVMIsDeclaration(function) == 0 {
                    functions.push(function);
                }
                function = LLVMGetNextFunction(function);
            }
        }
        functions
    }
}

/// Extrai `(class_id, campos)` de uma chamada constante a `@dartforge_object_new`.
///
/// # Safety
/// `instruction` precisa ser uma instrução viva de um módulo não consumido.
unsafe fn object_new_layout(instruction: LLVMValueRef) -> Option<(i64, i64)> {
    // SAFETY: as consultas abaixo só leem a instrução e seus operandos. `IsACall`
    // devolve nulo quando a instrução não é chamada, e o operando final de uma
    // chamada é o alvo — por isso os argumentos são lidos por índice explícito e
    // a aritmética de operandos é conferida antes.
    unsafe {
        if LLVMIsACallInst(instruction).is_null() {
            return None;
        }
        let callee = LLVMGetCalledValue(instruction);
        if callee.is_null() || value_name(callee) != "dartforge_object_new" {
            return None;
        }
        if LLVMGetNumOperands(instruction) < 3 {
            return None;
        }
        let class_id = LLVMGetOperand(instruction, 0);
        let fields = LLVMGetOperand(instruction, 1);
        if LLVMIsAConstantInt(class_id).is_null() || LLVMIsAConstantInt(fields).is_null() {
            return None;
        }
        Some((
            LLVMConstIntGetSExtValue(class_id) as i64,
            LLVMConstIntGetSExtValue(fields) as i64,
        ))
    }
}

/// Lê o nome de um valor global do LLVM.
///
/// # Safety
/// `value` precisa ser um valor vivo de um módulo não consumido.
unsafe fn value_name(value: LLVMValueRef) -> String {
    let mut len = 0usize;
    // SAFETY: `GetValueName2` devolve ponteiro e comprimento de uma string que
    // pertence ao LLVM e vive tanto quanto o valor; a cópia é feita aqui.
    unsafe {
        let name = LLVMGetValueName2(value, &mut len);
        if name.is_null() {
            return String::new();
        }
        String::from_utf8_lossy(std::slice::from_raw_parts(name.cast::<u8>(), len)).into_owned()
    }
}

/// Imprime um tipo do LLVM como texto de IR.
///
/// # Safety
/// `kind` precisa ser um tipo vivo do contexto de um módulo não consumido.
unsafe fn type_text(kind: LLVMTypeRef) -> String {
    // SAFETY: `PrintTypeToString` transfere a propriedade da string, liberada
    // aqui com `LLVMDisposeMessage`, como manda `llvm-c/Core.h`.
    unsafe { take_message(LLVMPrintTypeToString(kind)) }
}

/// Lista os tipos dos parâmetros de um tipo de função.
///
/// # Safety
/// `kind` precisa ser um tipo de função vivo.
unsafe fn param_types(kind: LLVMTypeRef) -> Vec<LLVMTypeRef> {
    // SAFETY: `CountParamTypes` informa o tamanho exato que `GetParamTypes`
    // escreve, e o vetor é dimensionado antes da escrita.
    unsafe {
        let count = LLVMCountParamTypes(kind) as usize;
        let mut types = vec![ptr::null_mut::<llvm_sys::LLVMType>(); count];
        if count > 0 {
            LLVMGetParamTypes(kind, types.as_mut_ptr());
        }
        types
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

/// Marca um símbolo absoluto como dado visível fora do módulo.
///
/// Sem `Callable`: é o endereço de uma célula de ponteiro, lida pelo trampolim
/// da entrada estável, e não código que alguém possa chamar.
fn exported_data() -> LLVMJITSymbolFlags {
    LLVMJITSymbolFlags {
        GenericFlags: LLVMJITSymbolGenericFlags::LLVMJITSymbolGenericFlagsExported as u8,
        TargetFlags: 0,
    }
}

/// A máquina-alvo do JIT: triple do host, CPU genérica `x86-64` e geração de
/// código **sem otimização**, o par do `clang -O0` do AOT. É a mesma para a
/// sessão ([`Lljit::new`]) e para o objeto em cache ([`compile_object`]): um
/// objeto compilado por uma e carregado na outra tem de ter o mesmo código.
///
/// O modelo de código é o padrão de JIT do LLVM, que no x86-64 evita
/// relocações de 32 bits entre o código gerado e o runtime do processo.
///
/// Devolve a posse do `TargetMachine` ao chamador.
fn target_machine() -> Result<LLVMTargetMachineRef, String> {
    initialize_native_target()?;
    // SAFETY: o triple e a mensagem de erro voltam para nós e são liberados
    // com `LLVMDisposeMessage`; o `TargetMachine` criado é devolvido.
    unsafe {
        let triple = LLVMGetDefaultTargetTriple();
        let mut target = ptr::null_mut();
        let mut message = ptr::null_mut();
        if LLVMGetTargetFromTriple(triple, &mut target, &mut message) != 0 {
            let detail = if message.is_null() { "alvo desconhecido".to_owned() } else { take_message(message) };
            LLVMDisposeMessage(triple);
            return Err(format!("o LLVM não tem alvo para o host: {detail}"));
        }
        let machine = LLVMCreateTargetMachine(
            target,
            triple,
            if cfg!(target_arch = "x86_64") { c"x86-64" } else { c"generic" }.as_ptr(),
            c"".as_ptr(),
            LLVMCodeGenOptLevel::LLVMCodeGenLevelNone,
            LLVMRelocMode::LLVMRelocDefault,
            LLVMCodeModel::LLVMCodeModelJITDefault,
        );
        LLVMDisposeMessage(triple);
        if machine.is_null() {
            return Err("o LLVM não criou a máquina-alvo do host".to_owned());
        }
        Ok(machine)
    }
}

/// Instância viva da `LLJIT` e sua `JITDylib` principal.
pub(crate) struct Lljit {
    handle: LLVMOrcLLJITRef,
    main: LLVMOrcJITDylibRef,
}

impl Lljit {
    /// Cria uma `LLJIT` para o triple do host e resolve sua `JITDylib` principal.
    ///
    /// A máquina-alvo é montada explicitamente, e não detectada: CPU genérica
    /// `x86-64` (a mesma que o `clang` do AOT assume sem `-march`) e geração de
    /// código **sem otimização** (`CodeGenLevelNone`), o par do `clang -O0` do
    /// ciclo de desenvolvimento. A detecção do host daria a CPU da máquina e
    /// `CodeGenLevelDefault` — código diferente do AOT e mais lento de gerar.
    /// O modelo de código é o padrão de JIT do LLVM, que no x86-64 evita
    /// relocações de 32 bits entre o código gerado e o runtime do processo.
    ///
    /// # Erros
    /// Falha quando o alvo nativo não está disponível ou a construção da
    /// `LLJIT` retorna erro, sempre com a mensagem original do LLVM anexada.
    pub(crate) fn new() -> Result<Self, String> {
        let machine = target_machine()?;
        let mut handle: LLVMOrcLLJITRef = ptr::null_mut();
        // SAFETY: cada objeto criado aqui é entregue ao seguinte, que assume a
        // propriedade: o `TargetMachine` ao JTMB, o JTMB ao builder e o builder
        // a `LLVMOrcCreateLLJIT` (que o consome inclusive quando falha).
        unsafe {
            let builder = LLVMOrcCreateLLJITBuilder();
            LLVMOrcLLJITBuilderSetJITTargetMachineBuilder(
                builder,
                LLVMOrcJITTargetMachineBuilderCreateFromTargetMachine(machine),
            );
            if let Some(message) = take_error(LLVMOrcCreateLLJIT(&mut handle, builder)) {
                return Err(message);
            }
        }
        if handle.is_null() {
            return Err("LLVM devolveu uma LLJIT nula sem diagnóstico".to_owned());
        }
        // SAFETY: `handle` acabou de ser criado com sucesso; a dylib principal
        // pertence à `LLJIT` e é válida enquanto esta estrutura viver.
        let main = unsafe { LLVMOrcLLJITGetMainJITDylib(handle) };
        Ok(Self { handle, main })
    }

    /// `target datalayout` e `target triple` que esta `LLJIT` impõe aos módulos.
    pub(crate) fn target(&self) -> (String, String) {
        // SAFETY: as duas strings pertencem à `LLJIT` viva e são copiadas já.
        unsafe {
            (
                CStr::from_ptr(LLVMOrcLLJITGetDataLayoutStr(self.handle))
                    .to_string_lossy()
                    .into_owned(),
                CStr::from_ptr(LLVMOrcLLJITGetTripleString(self.handle))
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    }

    /// Publica os endereços das funções de runtime como símbolos absolutos.
    ///
    /// Esta é a contrapartida JIT da ligação que o driver AOT faz com o
    /// runtime: em vez de um linker resolver `@dartforge_print_i64` num objeto,
    /// a `JITDylib` passa a conter esse nome apontando para a função Rust já
    /// carregada neste processo. A tabela é gerada da fonte do runtime
    /// ([`RUNTIME_SYMBOLS`]); junto vão os dados que a CRT estática daria ao
    /// executável AOT ([`crt_data_symbols`]).
    ///
    /// # Erros
    /// Falha se algum nome já estiver definido na dylib, caso em que a unidade
    /// de materialização é liberada sem alterar a sessão.
    pub(crate) fn define_runtime_symbols(&self) -> Result<(), String> {
        let names: Vec<(CString, u64)> = runtime_symbol_addresses()
            .into_iter()
            .map(|(name, address)| {
                (
                    CString::new(name).expect("nome de runtime sem byte nulo"),
                    address as u64,
                )
            })
            .collect();
        let pairs: Vec<(&CStr, u64)> =
            names.iter().map(|(name, address)| (name.as_c_str(), *address)).collect();
        self.define_absolute(&pairs, exported_callable())?;
        self.define_absolute(&crt_data_symbols(), exported_data())
    }

    /// Publica endereços de **dados** desta thread como símbolos absolutos.
    ///
    /// É como os ponteiros de implementação do hot reload entram na `JITDylib`:
    /// cada nome passa a designar o endereço de uma célula Rust, e o trampolim
    /// gerado lê essa célula a cada chamada. Os símbolos são exportados mas
    /// **não** marcados como `Callable`, porque são dado, não código — a
    /// distinção que `llvm-c/Orc.h` pede e que evita que um erro de nome
    /// transforme uma célula em alvo de chamada.
    ///
    /// # Erros
    /// Falha se algum nome já estiver definido na dylib; nesse caso a unidade de
    /// materialização é liberada e a sessão não muda.
    pub(crate) fn define_data_symbols(&self, pairs: &[(CString, u64)]) -> Result<(), String> {
        let borrowed: Vec<(&CStr, u64)> = pairs
            .iter()
            .map(|(name, address)| (name.as_c_str(), *address))
            .collect();
        self.define_absolute(&borrowed, exported_data())
    }

    /// Define uma lista de símbolos absolutos com as mesmas flags.
    ///
    /// # Erros
    /// Propaga o diagnóstico do LLVM, tipicamente definição duplicada.
    fn define_absolute(
        &self,
        symbols: &[(&CStr, u64)],
        flags: LLVMJITSymbolFlags,
    ) -> Result<(), String> {
        let mut pairs = Vec::with_capacity(symbols.len());
        for (name, address) in symbols {
            // SAFETY: `name` está vivo durante a chamada e `self.handle` é válido.
            // A entrada devolvida já vem retida e sua propriedade é transferida
            // para `LLVMOrcAbsoluteSymbols` abaixo, que a consome.
            let interned = unsafe { LLVMOrcLLJITMangleAndIntern(self.handle, name.as_ptr()) };
            pairs.push(LLVMOrcCSymbolMapPair {
                Name: interned,
                Sym: LLVMJITEvaluatedSymbol {
                    Address: *address,
                    Flags: LLVMJITSymbolFlags {
                        GenericFlags: flags.GenericFlags,
                        TargetFlags: flags.TargetFlags,
                    },
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

    /// Chama uma entrada estável cuja assinatura foi conferida aqui mesmo.
    ///
    /// A verificação é a razão de a função ser segura. O chamador informa o
    /// endereço e a assinatura que a sessão registrou para aquela entrada — lida
    /// do próprio IR por [`ParsedModule::signatures`] —, e só duas formas são
    /// aceitas: `i64 ()` e `i64 (i64)`. Qualquer outra vira erro, jamais uma
    /// chamada com ABI errada. É o mesmo raciocínio de [`Lljit::run_entry`], que
    /// não aceita nome de símbolo do chamador, aplicado a assinaturas com valor
    /// de retorno.
    ///
    /// O endereço precisa ser o de uma **entrada estável** (o trampolim), nunca
    /// o de uma implementação de geração: o trampolim não é descarregado, e é o
    /// que garante que uma chamada feita depois de uma recarga chegue à
    /// implementação nova.
    ///
    /// # Erros
    /// Assinatura fora das duas formas suportadas, ou argumento presente/ausente
    /// em desacordo com a aridade.
    pub(crate) fn call_stable(
        &self,
        address: u64,
        signature: &FunctionSignature,
        argument: Option<i64>,
    ) -> Result<i64, String> {
        if signature.var_arg || signature.ret != "i64" {
            return Err(format!(
                "a entrada estável tem assinatura {} e este crate só chama i64 () e i64 (i64)",
                signature.text()
            ));
        }
        let pointer = usize::try_from(address)
            .map_err(|_| "endereço da entrada estável não cabe em usize".to_owned())?
            as *const ();
        match (signature.params.as_slice(), argument) {
            ([], None) => {
                // SAFETY: a assinatura registrada para este endereço é `i64 ()`,
                // conferida acima, e o endereço veio de um `lookup` desta `LLJIT`
                // viva, cujo trampolim nunca é descarregado. `extern "C"` é a ABI
                // declarada no IR pelo emissor.
                Ok(unsafe { std::mem::transmute::<*const (), extern "C" fn() -> i64>(pointer)() })
            }
            ([param], Some(value)) if param == "i64" => {
                // SAFETY: idem, para a assinatura `i64 (i64)`.
                Ok(unsafe {
                    std::mem::transmute::<*const (), extern "C" fn(i64) -> i64>(pointer)(value)
                })
            }
            _ => Err(format!(
                "a entrada estável tem assinatura {} e o argumento informado não corresponde",
                signature.text()
            )),
        }
    }

    /// Resolve e executa a entrada do módulo, medindo as duas fases.
    ///
    /// A assinatura é segura de propósito: o nome do símbolo **não** vem do
    /// chamador. Resolver [`crate::ENTRY_SYMBOL`] nesta `LLJIT` viva e chamá-lo
    /// como `void(void)` é correto porque `crates/emit_native` emite
    /// `@dartforge_entry` exatamente com essa assinatura, e `crates/jit` só
    /// aceita IR desse emissor — a mesma hipótese que o driver AOT faz. Se o
    /// chamador pudesse escolher o nome, a função precisaria ser `unsafe`.
    ///
    /// Depois da entrada vem `finalizar_programa` do runtime (a mesma fonte do
    /// AOT), numa thread própria com `stack_bytes` de pilha: ver
    /// [`execute_program`]. `&self` garante que a `LLJIT` e o módulo da entrada
    /// continuam vivos até a thread terminar, porque remover exige `&mut`.
    ///
    /// Devolve `(lookup, código de saída, execute)`; `lookup` inclui a geração
    /// de código do módulo que define a entrada.
    ///
    /// # Erros
    /// Falha quando nenhum módulo da sessão define a entrada, quando a
    /// compilação sob demanda desse módulo falha, ou quando a thread do
    /// programa não pode ser criada.
    pub(crate) fn run_entry(
        &self,
        stack_bytes: usize,
        globals: &[&MutableGlobal],
    ) -> Result<(Duration, i32, Duration), String> {
        let phase = Instant::now();
        let address = self.lookup(crate::ENTRY_SYMBOL)?;
        let lookup = phase.elapsed();
        // Os estáticos da execução anterior (se houve) voltam a zero: cada
        // execução começa como um processo novo do AOT.
        for global in globals {
            self.zero_global(global)?;
        }
        let (code, execute) = execute_program(address, stack_bytes)?;
        Ok((lookup, code, execute))
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

// ─── Runtime ───────────────────────────────────────────────────────────────
//
// O runtime são os fragmentos de `crates/runtime/src`, a mesma fonte que o AOT
// compila com `rustc` avulso, aqui compilada como o módulo
// `dartforge_runtime::abi` (fonte única, plano do JIT §3.1). A tabela de
// símbolos é gerada pelo `build.rs` daquele crate a partir dos
// `#[unsafe(no_mangle)]` dos fragmentos.

/// Nomes do runtime publicados na sessão, na ordem da fonte.
pub(crate) const RUNTIME_SYMBOLS: &[&str] = dartforge_runtime::simbolos::NOMES;

/// Endereço de cada função do runtime, na ordem de [`RUNTIME_SYMBOLS`].
fn runtime_symbol_addresses() -> Vec<(&'static str, usize)> {
    dartforge_runtime::simbolos::tabela()
}

/// Executa o programa cuja entrada está em `address`, como o `main` do AOT.
///
/// O programa roda numa thread **nova**, com pilha de `stack_bytes`. Todo o
/// estado do runtime é `thread_local` (heap, classes registradas, exceção
/// pendente), então cada execução começa de um runtime limpo sem nenhum
/// `reset` a manter em sincronia com o runtime — e a pilha é a mesma que o
/// executável AOT recebe, para que recursão profunda falhe igual.
///
/// Depois da entrada vem `dartforge_runtime::abi::finalizar_programa`, o mesmo
/// código que o `main` C do AOT roda (exceção não capturada → 101, estatísticas
/// do coletor). O código que ela devolve é devolvido aqui; os finais que o
/// runtime trata com `process::exit` durante a execução — asserção de não
/// nulidade (101), teto do heap (255) — encerram **este processo**, como no
/// AOT. Por isso testes e o harness diferencial executam por subprocesso
/// (`dartforge-executar-ir`).
///
/// # Erros
/// Falha ao criar a thread, ou se a thread terminar em pânico fora do código
/// `extern "C"` (lá dentro um pânico aborta o processo, como no AOT).
pub(crate) fn execute_program(address: u64, stack_bytes: usize) -> Result<(i32, Duration), String> {
    let address = usize::try_from(address).map_err(|_| "endereço da entrada não cabe em usize".to_owned())?;
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .stack_size(stack_bytes)
            .spawn_scoped(scope, move || {
                let started = Instant::now();
                // SAFETY: `address` veio de `LLVMOrcLLJITLookup(dartforge_entry)`
                // numa `LLJIT` que o chamador mantém viva durante toda a thread
                // (`Lljit::run_entry` toma `&self` e só retorna depois do
                // `join`), e `@dartforge_entry` é emitido com a assinatura C
                // `void(void)`.
                unsafe { std::mem::transmute::<usize, extern "C" fn()>(address)() };
                let code = dartforge_runtime::abi::finalizar_programa();
                (code, started.elapsed())
            })
            .map_err(|erro| format!("não foi possível criar a thread do programa: {erro}"))?;
        handle
            .join()
            .map_err(|_| "a thread do programa terminou em pânico".to_owned())
    })
}

/// Nomes externos que o IR pode **declarar** sem que o runtime os defina.
///
/// São funções da CRT que o processo já carrega (`vcruntime140`/`ucrtbase`) e
/// que um emissor de LLVM IR pode chamar diretamente: cópia de memória e a
/// biblioteca matemática de `double`. Ficam numa lista explícita, e não num
/// «qualquer coisa que o processo tenha», para que um nome desconhecido falhe
/// alto em vez de ser resolvido por acaso para algo homônimo. Símbolos que o
/// **gerador de código** introduz sem aparecer no IR (`__chkstk`, `memcpy` de
/// intrínsecos) não passam por esta lista: a `LLJIT` os resolve no processo.
pub(crate) const CRT_SYMBOLS: &[&str] = &[
    "memcpy", "memmove", "memset", "memcmp", "fmod", "pow", "sqrt", "floor", "ceil", "trunc",
    "round", "exp", "log", "log10", "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
];

/// Diz se `name` é um externo que a sessão sabe resolver sem outro módulo.
pub(crate) fn is_known_external(name: &str) -> bool {
    RUNTIME_SYMBOLS.contains(&name) || CRT_SYMBOLS.contains(&name) || name.starts_with("llvm.")
}

/// Valor de `_fltused`, o marcador que o MSVC exige de quem usa ponto flutuante.
///
/// O gerador de código COFF referencia `_fltused` em todo objeto que usa
/// `double`. No AOT ele vem da CRT estática ligada ao executável; num processo
/// Rust ele não é exportado por nenhuma DLL, então a sessão o publica. O valor é
/// o mesmo da CRT; só o endereço importa, ninguém o lê.
static FLTUSED: i32 = 0x9875;

/// Símbolos de dado que a sessão publica além do runtime.
fn crt_data_symbols() -> Vec<(&'static CStr, u64)> {
    vec![(c"_fltused", std::ptr::addr_of!(FLTUSED) as u64)]
}

/// Versão da `LLVM-C.dll` efetivamente carregada, `(major, minor, patch)`.
pub(crate) fn llvm_version() -> (u32, u32, u32) {
    let (mut major, mut minor, mut patch) = (0, 0, 0);
    // SAFETY: os três destinos são válidos; a função só escreve neles.
    unsafe { llvm_sys::core::LLVMGetVersion(&mut major, &mut minor, &mut patch) };
    (major, minor, patch)
}

// ─── Estado mutável do módulo e objetos em cache ─────────────────────────

/// Uma global mutável do módulo que precisa voltar a zero antes de cada
/// execução: nome e tamanho em bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MutableGlobal {
    pub(crate) name: String,
    pub(crate) size: usize,
}

impl ParsedModule {
    /// Prepara as globais mutáveis para serem reiniciadas entre execuções.
    ///
    /// O emissor guarda os estáticos preguiçosos em globais do módulo
    /// (`@dfg_<id>` e `@dfg_<id>_ok`, `internal`, iniciadas com zero). Numa
    /// sessão que roda o mesmo módulo várias vezes, a segunda execução veria
    /// os estáticos já «inicializados» com handles do heap da execução
    /// anterior, que não existe mais. Cada execução precisa começar do zero,
    /// como um processo novo do AOT.
    ///
    /// Para que a sessão consiga achar essas globais, a ligação delas passa de
    /// `internal` para externa **nesta cópia** do módulo. O IR emitido não
    /// muda. Só globais com inicializador nulo são aceitas: é o que o emissor
    /// produz, e reiniciar é escrever zeros. Outra forma é recusada com o
    /// nome, em vez de uma reinicialização errada em silêncio.
    pub(crate) fn prepare_mutable_globals(&self) -> Result<Vec<MutableGlobal>, String> {
        use llvm_sys::LLVMLinkage;
        let mut globals = Vec::new();
        // SAFETY: travessia pela API do LLVM sobre o módulo vivo, terminada no
        // primeiro nulo; a troca de ligação é feita antes de o módulo ser
        // entregue à `LLJIT` ou emitido.
        unsafe {
            let mut global = llvm_sys::core::LLVMGetFirstGlobal(self.module);
            while !global.is_null() {
                let next = llvm_sys::core::LLVMGetNextGlobal(global);
                if LLVMIsDeclaration(global) == 0 && llvm_sys::core::LLVMIsGlobalConstant(global) == 0 {
                    let name = value_name(global);
                    let init = llvm_sys::core::LLVMGetInitializer(global);
                    if init.is_null() || llvm_sys::core::LLVMIsNull(init) == 0 {
                        return Err(format!(
                            "a global mutável @{name} não começa em zero; a sessão não sabe reiniciá-la entre execuções"
                        ));
                    }
                    let kind = type_text(LLVMGlobalGetValueType(global));
                    let size = match kind.as_str() {
                        "i1" | "i8" => 1,
                        "i16" => 2,
                        "i32" | "float" => 4,
                        "i64" | "double" | "ptr" => 8,
                        outro => {
                            return Err(format!(
                                "a global mutável @{name} tem tipo {outro}, que a sessão não sabe reiniciar"
                            ));
                        }
                    };
                    let linkage = llvm_sys::core::LLVMGetLinkage(global);
                    if matches!(linkage, LLVMLinkage::LLVMInternalLinkage | LLVMLinkage::LLVMPrivateLinkage) {
                        llvm_sys::core::LLVMSetLinkage(global, LLVMLinkage::LLVMExternalLinkage);
                    }
                    globals.push(MutableGlobal { name, size });
                }
                global = next;
            }
        }
        Ok(globals)
    }
}

impl Lljit {
    /// Escreve zeros na global mutável, já materializada.
    ///
    /// # Erros
    /// Falha se o símbolo não puder ser resolvido.
    pub(crate) fn zero_global(&self, global: &MutableGlobal) -> Result<(), String> {
        let address = self.lookup(&global.name)?;
        // SAFETY: `address` foi resolvido nesta `LLJIT` viva para uma global de
        // dado com exatamente `size` bytes (lido do tipo dela no IR). Nenhum
        // código gerado está executando: `run_entry` só zera antes de criar a
        // thread do programa, e a sessão é emprestada por `&self`.
        unsafe {
            std::ptr::write_bytes(usize::try_from(address).unwrap_or(usize::MAX) as *mut u8, 0, global.size);
        }
        Ok(())
    }

    /// Entrega à `LLJIT` um objeto já compilado, sob o rastreador informado.
    ///
    /// # Erros
    /// Propaga a mensagem do LLVM (objeto inválido, símbolo duplicado).
    pub(crate) fn add_object(&self, tracker: &ResourceTracker, object: &[u8], name: &str) -> Result<(), String> {
        let name = CString::new(name).map_err(|_| "nome do módulo contém byte nulo".to_owned())?;
        // SAFETY: o buffer é uma cópia de `object` e passa a pertencer à
        // `LLJIT` na chamada seguinte, inclusive quando ela devolve erro.
        unsafe {
            let buffer = LLVMCreateMemoryBufferWithMemoryRangeCopy(
                object.as_ptr().cast::<c_char>(),
                object.len(),
                name.as_ptr(),
            );
            match take_error(llvm_sys::orc2::lljit::LLVMOrcLLJITAddObjectFileWithRT(
                self.handle,
                tracker.handle,
                buffer,
            )) {
                Some(message) => Err(message),
                None => Ok(()),
            }
        }
    }
}

/// O que [`compile_object`] produz: o objeto e o que a sessão precisa saber
/// dele sem o IR (externos, definições, globais a reiniciar, alvo).
pub(crate) struct ObjectParts {
    pub(crate) object: Vec<u8>,
    pub(crate) declarations: Vec<String>,
    pub(crate) signatures: Vec<FunctionSignature>,
    pub(crate) layouts: Vec<(i64, i64)>,
    pub(crate) globals: Vec<MutableGlobal>,
    pub(crate) target: (String, String),
}

/// Compila IR textual em objeto COFF com a máquina-alvo do JIT, sem sessão.
///
/// É a peça do cache de módulos: compila uma vez e carrega o objeto em
/// quantas sessões quiser ([`Lljit::add_object`]), sem pagar de novo a análise
/// e a geração de código. O código é o mesmo que a sessão geraria, porque a
/// máquina-alvo é a mesma ([`target_machine`]).
///
/// # Erros
/// IR inválido (inclusive pelo verificador), global que não se reinicia, ou
/// falha do gerador de código.
pub(crate) fn compile_object(name: &str, ir: &str) -> Result<ObjectParts, String> {
    let parsed = parse_module(name, ir)?;
    let globals = parsed.prepare_mutable_globals()?;
    let declarations = parsed.declarations();
    let signatures = parsed.signatures();
    let layouts = parsed.class_layouts();
    let machine = target_machine()?;
    // SAFETY: `machine` é nosso até o `Dispose` no fim; o módulo pertence a
    // `parsed`, vivo durante a emissão; a camada de dados criada para um
    // módulo sem layout é liberada logo depois de copiada para ele; o buffer
    // devolvido é copiado e liberado.
    let object = unsafe {
        if CStr::from_ptr(llvm_sys::core::LLVMGetDataLayoutStr(parsed.module)).to_bytes().is_empty() {
            let dados = llvm_sys::target_machine::LLVMCreateTargetDataLayout(machine);
            llvm_sys::target::LLVMSetModuleDataLayout(parsed.module, dados);
            llvm_sys::target::LLVMDisposeTargetData(dados);
        }
        let mut message = ptr::null_mut();
        let mut buffer = ptr::null_mut();
        let falhou = LLVMTargetMachineEmitToMemoryBuffer(
            machine,
            parsed.module,
            LLVMCodeGenFileType::LLVMObjectFile,
            &mut message,
            &mut buffer,
        ) != 0;
        let resultado = if falhou {
            Err(if message.is_null() { "o gerador de código falhou".to_owned() } else { take_message(message) })
        } else {
            let inicio = llvm_sys::core::LLVMGetBufferStart(buffer).cast::<u8>();
            let tamanho = llvm_sys::core::LLVMGetBufferSize(buffer);
            let bytes = std::slice::from_raw_parts(inicio, tamanho).to_vec();
            LLVMDisposeMemoryBuffer(buffer);
            Ok(bytes)
        };
        LLVMDisposeTargetMachine(machine);
        resultado
    }?;
    let target = parsed.target();
    Ok(ObjectParts { object, declarations, signatures, layouts, globals, target })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A tabela gerada cobre exatamente os nomes anunciados, com endereço real.
    #[test]
    fn symbol_table_matches_the_announced_names() {
        let addresses = runtime_symbol_addresses();
        assert_eq!(addresses.len(), RUNTIME_SYMBOLS.len());
        for (index, (name, address)) in addresses.iter().enumerate() {
            assert_eq!(*name, RUNTIME_SYMBOLS[index]);
            assert_ne!(*address, 0);
        }
        assert!(!RUNTIME_SYMBOLS.contains(&"main"));
        assert!(!RUNTIME_SYMBOLS.contains(&"dartforge_entry"));
    }
    /// Externos: runtime, CRT listada e intrínsecos passam; o resto não.
    #[test]
    fn only_listed_externals_are_known() {
        assert!(is_known_external("dartforge_print_i64"));
        assert!(is_known_external("fmod"));
        assert!(is_known_external("llvm.memcpy.p0.p0.i64"));
        assert!(!is_known_external("sqlite3_open"));
        assert!(!is_known_external("dartforge_entry"));
    }
}
