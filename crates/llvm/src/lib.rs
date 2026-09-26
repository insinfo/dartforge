//! O LLVM dentro do processo do dartforge: a ligação com a biblioteca (o
//! `build.rs`), a inicialização do alvo nativo, uma vez por processo, e o
//! **gerador de código do AOT**: LLVM IR em texto → objeto nativo ou bitcode
//! para a otimização na ligação (LTO), sem o `clang` na máquina de quem usa o
//! dartforge.
//!
//! [`gerar`] faz o que o `clang -x ir -c` fazia no driver do AOT: lê o
//! módulo, roda o pipeline de otimização do nível pedido (o mesmo
//! `default<On>` do `PassBuilder` que o Clang roda) e emite pela máquina-alvo
//! do hospedeiro, com a CPU que o Clang assume sem `-march`. O JIT
//! (`crates/jit`) usa a mesma biblioteca e a mesma inicialização
//! ([`inicializar_alvo_nativo`]).
//!
//! # Por que `unsafe`
//!
//! Toda chamada da API C do LLVM é `unsafe`, e a posse dos recursos
//! (contexto, módulo, máquina-alvo, buffers, mensagens) segue o contrato de
//! `llvm-c/*.h`, não o borrow checker. Cada recurso aqui pertence a um valor
//! Rust com `Drop` ([`Contexto`], [`Modulo`], [`MaquinaAlvo`]); nada cru sai
//! deste arquivo: a API pública só troca `&str`, `Vec<u8>` e `String`.
//
// Único `allow` do crate: qualquer `unsafe` novo fora daqui é erro de
// compilação (`unsafe_code = "deny"` do workspace).
#![allow(unsafe_code)]

use llvm_sys::bit_writer::LLVMWriteBitcodeToMemoryBuffer;
use llvm_sys::core::{
    LLVMContextCreate, LLVMContextDispose, LLVMCreateMemoryBufferWithMemoryRangeCopy, LLVMDisposeMemoryBuffer,
    LLVMDisposeMessage, LLVMDisposeModule, LLVMGetBufferSize, LLVMGetBufferStart, LLVMGetDataLayoutStr,
    LLVMGetTarget, LLVMGetVersion, LLVMSetTarget,
};
use llvm_sys::error::{LLVMDisposeErrorMessage, LLVMErrorRef, LLVMGetErrorMessage};
use llvm_sys::ir_reader::LLVMParseIRInContext2;
use llvm_sys::prelude::{LLVMContextRef, LLVMMemoryBufferRef, LLVMModuleRef};
use llvm_sys::target::{
    LLVM_InitializeNativeAsmPrinter, LLVM_InitializeNativeTarget, LLVMDisposeTargetData, LLVMSetModuleDataLayout,
};
use llvm_sys::target_machine::{
    LLVMCodeGenFileType, LLVMCodeGenOptLevel, LLVMCodeModel, LLVMCreateTargetDataLayout, LLVMCreateTargetMachine,
    LLVMDisposeTargetMachine, LLVMGetDefaultTargetTriple, LLVMGetTargetFromTriple, LLVMRelocMode,
    LLVMTargetMachineEmitToMemoryBuffer, LLVMTargetMachineRef,
};
use llvm_sys::transforms::pass_builder::{
    LLVMCreatePassBuilderOptions, LLVMDisposePassBuilderOptions, LLVMRunPasses,
};
use std::ffi::{CStr, CString, c_char};
use std::ptr;
use std::sync::OnceLock;

/// Registra o alvo nativo e o `AsmPrinter` dele, uma única vez por processo.
///
/// As rotinas do LLVM não são seguras sob concorrência; o [`OnceLock`]
/// garante execução única e memoriza a falha.
///
/// # Erros
/// Quando o LLVM ligado não tem o backend da arquitetura do hospedeiro.
pub fn inicializar_alvo_nativo() -> Result<(), String> {
    static ESTADO: OnceLock<Result<(), String>> = OnceLock::new();
    ESTADO
        .get_or_init(|| {
            // SAFETY: `get_or_init` garante execução única e exclusiva; as
            // rotinas só registram o alvo em tabelas globais do LLVM.
            let falhou = unsafe { LLVM_InitializeNativeTarget() != 0 || LLVM_InitializeNativeAsmPrinter() != 0 };
            if falhou { Err("o LLVM não tem backend nativo para esta arquitetura".to_owned()) } else { Ok(()) }
        })
        .clone()
}

/// A versão do LLVM ligado (`22.1.8`).
pub fn versao() -> String {
    let (mut maior, mut menor, mut correcao) = (0u32, 0u32, 0u32);
    // SAFETY: a função só escreve nos três inteiros.
    unsafe { LLVMGetVersion(&mut maior, &mut menor, &mut correcao) };
    format!("{maior}.{menor}.{correcao}")
}

/// O que [`gerar`] produz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    /// Objeto nativo do hospedeiro (COFF, ELF ou Mach-O).
    Objeto,
    /// Bitcode para a otimização na ligação (LTO do `lld`): o módulo passa
    /// pelo pipeline de pré-ligação (`lto-pre-link<O2>`) e o resto da
    /// otimização acontece com o programa inteiro à vista.
    Bitcode,
}

/// Como [`gerar`] compila um módulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opcoes {
    /// `-O2` (o `default<O2>` e o gerador de código no nível padrão) em vez
    /// de `-O0`.
    pub otimizar: bool,
    pub formato: Formato,
}

impl Opcoes {
    /// O que entra na chave de um objeto em cache: dois módulos iguais com
    /// opções de mesma descrição dão os mesmos bytes.
    pub fn descricao(&self) -> String {
        format!(
            "{} {}",
            if self.otimizar { "O2" } else { "O0" },
            match self.formato {
                Formato::Objeto => "objeto",
                Formato::Bitcode => "bitcode-lto",
            }
        )
    }
}

/// A identidade do gerador para as chaves de cache: a versão do LLVM, o
/// triple e a CPU do hospedeiro. Outro LLVM ou outra máquina-alvo não
/// reaproveitam objeto deste.
pub fn identidade() -> Result<String, String> {
    static ID: OnceLock<Result<String, String>> = OnceLock::new();
    ID.get_or_init(|| {
        inicializar_alvo_nativo()?;
        let triple = triple_padrao();
        Ok(format!("llvm-embutido {} {} {}", versao(), triple, cpu_padrao(&triple)))
    })
    .clone()
}

/// Compila o LLVM IR em texto `ir` (o `nome` aparece nas mensagens e como
/// identificador do módulo) e devolve os bytes do objeto ou do bitcode.
///
/// O módulo sem `target triple` (o emissor o omite no macOS, onde o triple
/// leva a versão do sistema) recebe o do hospedeiro, como o Clang faria; sem
/// `target datalayout`, o da máquina-alvo.
///
/// # Erros
/// IR que o leitor recusa, alvo sem backend, falha do pipeline ou do gerador
/// de código — a mensagem é a do LLVM.
pub fn gerar(nome: &str, ir: &str, opcoes: &Opcoes) -> Result<Vec<u8>, String> {
    inicializar_alvo_nativo()?;
    let contexto = Contexto::novo();
    let modulo = contexto.ler(nome, ir)?;
    let triple = modulo.triple().unwrap_or_else(triple_padrao);
    let maquina = MaquinaAlvo::do_triple(&triple, opcoes.otimizar)?;
    modulo.completar_alvo(&triple, &maquina);
    let pipeline = match (opcoes.formato, opcoes.otimizar) {
        (Formato::Bitcode, _) => "lto-pre-link<O2>",
        (Formato::Objeto, true) => "default<O2>",
        (Formato::Objeto, false) => "default<O0>",
    };
    modulo.otimizar(pipeline, &maquina)?;
    match opcoes.formato {
        Formato::Objeto => maquina.emitir_objeto(&modulo),
        Formato::Bitcode => Ok(modulo.bitcode()),
    }
}

/// O triple do hospedeiro, como o LLVM o descreve (no macOS, com a versão do
/// sistema em execução).
fn triple_padrao() -> String {
    // SAFETY: a string devolvida é nossa e liberada por `tomar_mensagem`.
    unsafe { tomar_mensagem(LLVMGetDefaultTargetTriple()) }
}

/// A CPU que o Clang assume sem `-march` para o triple: `x86-64` no x86-64,
/// `apple-m1` no arm64 da Apple, `generic` nos demais.
fn cpu_padrao(triple: &str) -> &'static str {
    let arquitetura = triple.split('-').next().unwrap_or_default();
    let apple = triple.contains("-apple-");
    match arquitetura {
        "x86_64" => "x86-64",
        "arm64" | "aarch64" if apple => "apple-m1",
        _ => "generic",
    }
}

/// Copia uma mensagem `char*` do LLVM e libera o original.
///
/// # Safety
/// `mensagem` é não nula e veio de uma API que transfere a posse ao chamador.
unsafe fn tomar_mensagem(mensagem: *mut c_char) -> String {
    // SAFETY: garantido pelo chamador.
    unsafe {
        let texto = CStr::from_ptr(mensagem).to_string_lossy().into_owned();
        LLVMDisposeMessage(mensagem);
        texto
    }
}

/// Consome um `LLVMErrorRef` e devolve a mensagem (`None` sem erro).
fn tomar_erro(erro: LLVMErrorRef) -> Option<String> {
    if erro.is_null() {
        return None;
    }
    // SAFETY: `erro` é não nulo e não foi consumido; a mensagem devolvida é
    // nossa e sai com `LLVMDisposeErrorMessage` (`llvm-c/Error.h`).
    unsafe {
        let mensagem = LLVMGetErrorMessage(erro);
        let texto = CStr::from_ptr(mensagem).to_string_lossy().into_owned();
        LLVMDisposeErrorMessage(mensagem);
        Some(texto)
    }
}

/// Copia e libera um `MemoryBuffer`.
///
/// # Safety
/// `buffer` é não nulo e nosso.
unsafe fn tomar_buffer(buffer: LLVMMemoryBufferRef) -> Vec<u8> {
    // SAFETY: garantido pelo chamador; início e tamanho descrevem o buffer
    // vivo até o `Dispose`.
    unsafe {
        let inicio = LLVMGetBufferStart(buffer).cast::<u8>();
        let bytes = std::slice::from_raw_parts(inicio, LLVMGetBufferSize(buffer)).to_vec();
        LLVMDisposeMemoryBuffer(buffer);
        bytes
    }
}

/// Um `LLVMContext` próprio: cada geração tem o seu, e gerações em threads
/// diferentes não compartilham estado do LLVM.
struct Contexto(LLVMContextRef);

impl Contexto {
    fn novo() -> Self {
        // SAFETY: cria um contexto novo, liberado no `Drop`.
        Contexto(unsafe { LLVMContextCreate() })
    }

    /// Lê o IR em texto. O módulo devolvido vive dentro deste contexto (o
    /// `Modulo` empresta `self`).
    fn ler(&self, nome: &str, ir: &str) -> Result<Modulo<'_>, String> {
        let nome_c = CString::new(nome).map_err(|_| format!("nome de módulo com NUL: {nome:?}"))?;
        // SAFETY: o buffer copia os bytes do IR; `LLVMParseIRInContext2` não o
        // consome, e ele é liberado logo depois. O módulo criado é nosso e
        // vai para o `Modulo`; a mensagem de erro, se houver, é nossa.
        unsafe {
            let buffer = LLVMCreateMemoryBufferWithMemoryRangeCopy(ir.as_ptr().cast(), ir.len(), nome_c.as_ptr());
            let mut modulo = ptr::null_mut();
            let mut mensagem = ptr::null_mut();
            let falhou = LLVMParseIRInContext2(self.0, buffer, &mut modulo, &mut mensagem) != 0;
            LLVMDisposeMemoryBuffer(buffer);
            if falhou {
                let detalhe = if mensagem.is_null() { "IR inválido".to_owned() } else { tomar_mensagem(mensagem) };
                return Err(format!("o LLVM recusou o IR de {nome}: {detalhe}"));
            }
            Ok(Modulo { m: modulo, _contexto: self })
        }
    }
}

impl Drop for Contexto {
    fn drop(&mut self) {
        // SAFETY: os módulos deste contexto emprestam `self` e já foram
        // liberados; o contexto é liberado uma única vez.
        unsafe { LLVMContextDispose(self.0) }
    }
}

/// Um módulo lido, dono do `LLVMModuleRef`.
struct Modulo<'c> {
    m: LLVMModuleRef,
    _contexto: &'c Contexto,
}

impl Modulo<'_> {
    /// O `target triple` do módulo, se declarado.
    fn triple(&self) -> Option<String> {
        // SAFETY: a string pertence ao módulo, vivo; é copiada.
        let t = unsafe { CStr::from_ptr(LLVMGetTarget(self.m)) }.to_string_lossy().into_owned();
        (!t.is_empty()).then_some(t)
    }

    /// Completa o triple e a camada de dados que o módulo não declara.
    fn completar_alvo(&self, triple: &str, maquina: &MaquinaAlvo) {
        // SAFETY: o módulo e a máquina estão vivos; a camada de dados criada
        // é copiada para o módulo e liberada; o triple é copiado pelo LLVM.
        unsafe {
            if self.triple().is_none()
                && let Ok(t) = CString::new(triple)
            {
                LLVMSetTarget(self.m, t.as_ptr());
            }
            if CStr::from_ptr(LLVMGetDataLayoutStr(self.m)).to_bytes().is_empty() {
                let dados = LLVMCreateTargetDataLayout(maquina.0);
                LLVMSetModuleDataLayout(self.m, dados);
                LLVMDisposeTargetData(dados);
            }
        }
    }

    /// Roda o pipeline textual do `PassBuilder` (`default<O2>`…) com a
    /// máquina-alvo (custos e legalidade do alvo, como no Clang).
    fn otimizar(&self, pipeline: &str, maquina: &MaquinaAlvo) -> Result<(), String> {
        let texto = CString::new(pipeline).expect("pipeline sem NUL");
        // SAFETY: módulo e máquina vivos; as opções são criadas e liberadas
        // aqui; o erro devolvido é consumido por `tomar_erro`.
        let erro = unsafe {
            let opcoes = LLVMCreatePassBuilderOptions();
            let erro = LLVMRunPasses(self.m, texto.as_ptr(), maquina.0, opcoes);
            LLVMDisposePassBuilderOptions(opcoes);
            erro
        };
        match tomar_erro(erro) {
            Some(e) => Err(format!("o pipeline {pipeline} do LLVM falhou: {e}")),
            None => Ok(()),
        }
    }

    fn bitcode(&self) -> Vec<u8> {
        // SAFETY: o buffer criado é nosso e sai por `tomar_buffer`.
        unsafe { tomar_buffer(LLVMWriteBitcodeToMemoryBuffer(self.m)) }
    }
}

impl Drop for Modulo<'_> {
    fn drop(&mut self) {
        // SAFETY: o módulo é nosso e liberado uma única vez, antes do
        // contexto (o empréstimo garante a ordem).
        unsafe { LLVMDisposeModule(self.m) }
    }
}

/// A máquina-alvo de uma geração.
struct MaquinaAlvo(LLVMTargetMachineRef);

impl MaquinaAlvo {
    /// A máquina do `triple` com a CPU padrão do Clang ([`cpu_padrao`]).
    /// Código independente de posição fora do Windows: os executáveis do
    /// Linux são PIE (o padrão do Clang) e as bibliotecas compartilhadas do
    /// SDK o exigem; o Mach-O é sempre PIC.
    fn do_triple(triple: &str, otimizar: bool) -> Result<Self, String> {
        let triple_c = CString::new(triple).map_err(|_| format!("triple com NUL: {triple:?}"))?;
        let cpu = CString::new(cpu_padrao(triple)).expect("CPU sem NUL");
        let windows = triple.contains("windows");
        // SAFETY: o alvo devolvido é uma referência estática do registro do
        // LLVM; a mensagem de erro é nossa; a máquina criada vai para o
        // `MaquinaAlvo`, que a libera.
        unsafe {
            let mut alvo = ptr::null_mut();
            let mut mensagem = ptr::null_mut();
            if LLVMGetTargetFromTriple(triple_c.as_ptr(), &mut alvo, &mut mensagem) != 0 {
                let detalhe = if mensagem.is_null() { "alvo desconhecido".to_owned() } else { tomar_mensagem(mensagem) };
                return Err(format!("o LLVM não tem o alvo {triple}: {detalhe}"));
            }
            let maquina = LLVMCreateTargetMachine(
                alvo,
                triple_c.as_ptr(),
                cpu.as_ptr(),
                c"".as_ptr(),
                if otimizar { LLVMCodeGenOptLevel::LLVMCodeGenLevelDefault } else { LLVMCodeGenOptLevel::LLVMCodeGenLevelNone },
                if windows { LLVMRelocMode::LLVMRelocDefault } else { LLVMRelocMode::LLVMRelocPIC },
                LLVMCodeModel::LLVMCodeModelDefault,
            );
            if maquina.is_null() {
                return Err(format!("o LLVM não criou a máquina-alvo de {triple}"));
            }
            Ok(MaquinaAlvo(maquina))
        }
    }

    fn emitir_objeto(&self, modulo: &Modulo<'_>) -> Result<Vec<u8>, String> {
        // SAFETY: máquina e módulo vivos; a mensagem de erro e o buffer
        // devolvidos são nossos e liberados.
        unsafe {
            let mut mensagem = ptr::null_mut();
            let mut buffer = ptr::null_mut();
            let falhou = LLVMTargetMachineEmitToMemoryBuffer(
                self.0,
                modulo.m,
                LLVMCodeGenFileType::LLVMObjectFile,
                &mut mensagem,
                &mut buffer,
            ) != 0;
            if falhou {
                return Err(if mensagem.is_null() { "o gerador de código do LLVM falhou".to_owned() } else { tomar_mensagem(mensagem) });
            }
            Ok(tomar_buffer(buffer))
        }
    }
}

impl Drop for MaquinaAlvo {
    fn drop(&mut self) {
        // SAFETY: a máquina é nossa e liberada uma única vez.
        unsafe { LLVMDisposeTargetMachine(self.0) }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const IR: &str = "define i32 @soma(i32 %a, i32 %b) {\n  %s = add i32 %a, %b\n  ret i32 %s\n}\n";

    #[test]
    fn gera_objeto_e_bitcode() {
        for otimizar in [false, true] {
            let obj = gerar("soma", IR, &Opcoes { otimizar, formato: Formato::Objeto }).unwrap();
            let magico = &obj[..4];
            // ELF, Mach-O (64 bits) ou COFF x86-64/arm64.
            assert!(
                magico == b"\x7fELF" || magico == [0xcf, 0xfa, 0xed, 0xfe] || obj[..2] == [0x64, 0x86] || obj[..2] == [0x64, 0xaa],
                "cabeçalho inesperado: {magico:x?}"
            );
        }
        let bc = gerar("soma", IR, &Opcoes { otimizar: true, formato: Formato::Bitcode }).unwrap();
        assert_eq!(&bc[..4], b"BC\xc0\xde");
    }

    #[test]
    fn mesmo_ir_mesmos_bytes() {
        let op = Opcoes { otimizar: true, formato: Formato::Objeto };
        assert_eq!(gerar("a", IR, &op).unwrap(), gerar("a", IR, &op).unwrap());
    }

    #[test]
    fn ir_invalido_da_erro_com_a_mensagem_do_llvm() {
        let e = gerar("ruim", "define i32 @f( {", &Opcoes { otimizar: false, formato: Formato::Objeto }).unwrap_err();
        assert!(e.contains("ruim"), "{e}");
    }

    #[test]
    fn cpu_do_clang() {
        assert_eq!(cpu_padrao("x86_64-pc-windows-msvc"), "x86-64");
        assert_eq!(cpu_padrao("arm64-apple-darwin24.1.0"), "apple-m1");
        assert_eq!(cpu_padrao("aarch64-unknown-linux-gnu"), "generic");
    }
}
