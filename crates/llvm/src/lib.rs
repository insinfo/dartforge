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

pub mod conferir_rs4gc;

use llvm_sys::bit_writer::LLVMWriteBitcodeToMemoryBuffer;
use llvm_sys::core::{
    LLVMConstNull, LLVMContextCreate, LLVMContextDispose, LLVMCreateMemoryBufferWithMemoryRangeCopy, LLVMDeleteFunction,
    LLVMDisposeMemoryBuffer, LLVMDisposeMessage, LLVMDisposeModule, LLVMDisposeOperandBundle, LLVMGetBufferSize,
    LLVMGetBufferStart, LLVMGetDataLayoutStr, LLVMGetFirstBasicBlock, LLVMGetFirstFunction, LLVMGetFirstInstruction,
    LLVMGetFirstUse, LLVMGetGC, LLVMGetInstructionOpcode, LLVMGetNamedFunction, LLVMGetNextBasicBlock, LLVMGetNextFunction,
    LLVMGetNextInstruction, LLVMGetNumArgOperands, LLVMGetNumOperandBundleArgs, LLVMGetNumOperandBundles, LLVMGetNumOperands,
    LLVMGetOperand, LLVMGetOperandBundleAtIndex, LLVMGetOperandBundleTag, LLVMGetTarget, LLVMGetVersion, LLVMIsACallInst,
    LLVMIsAConstantExpr, LLVMIsAConstantInt, LLVMIsAGlobalValue, LLVMIsAInstruction, LLVMIsAInvokeInst, LLVMPrintModuleToString,
    LLVMSetDataLayout, LLVMSetOperand, LLVMSetTarget, LLVMTypeOf, LLVMConstIntGetZExtValue, LLVMGetCalledValue, LLVMGetValueName2,
    LLVMConstInt,
};
use llvm_sys::LLVMOpcode;
use llvm_sys::error::{LLVMDisposeErrorMessage, LLVMErrorRef, LLVMGetErrorMessage};
use llvm_sys::ir_reader::LLVMParseIRInContext2;
use llvm_sys::support::LLVMParseCommandLineOptions;
use llvm_sys::prelude::{LLVMContextRef, LLVMMemoryBufferRef, LLVMModuleRef};
use llvm_sys::target::{
    LLVM_InitializeNativeAsmParser, LLVM_InitializeNativeAsmPrinter, LLVM_InitializeNativeTarget, LLVMDisposeTargetData, LLVMSetModuleDataLayout,
};
use llvm_sys::target_machine::{
    LLVMCodeGenFileType, LLVMCodeGenOptLevel, LLVMCodeModel, LLVMCreateTargetDataLayout, LLVMCreateTargetMachine,
    LLVMDisposeTargetMachine, LLVMGetDefaultTargetTriple, LLVMGetHostCPUFeatures, LLVMGetHostCPUName, LLVMGetTargetFromTriple, LLVMRelocMode,
    LLVMTargetMachineEmitToMemoryBuffer, LLVMTargetMachineRef,
};
use llvm_sys::transforms::pass_builder::{
    LLVMCreatePassBuilderOptions, LLVMDisposePassBuilderOptions, LLVMRunPasses,
};
use std::ffi::{CStr, CString, c_char};
use std::ptr;
use std::sync::OnceLock;

/// Registra o alvo nativo, o `AsmPrinter` e o `AsmParser` dele, uma única
/// vez por processo. O `AsmParser` é o que monta o `module asm` e o `asm`
/// em linha no objeto (a CRT mínima do Windows, `ligador_windows.rs`); sem
/// ele o LLVM aborta o processo ("Inline asm not supported by this
/// streamer").
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
            let falhou = unsafe {
                LLVM_InitializeNativeTarget() != 0
                    || LLVM_InitializeNativeAsmPrinter() != 0
                    || LLVM_InitializeNativeAsmParser() != 0
            };
            if falhou {
                return Err("o LLVM não tem backend nativo para esta arquitetura".to_owned());
            }
            // As opções do gerador de código, globais ao processo e lidas
            // uma vez (o `cl::opt` recusa a segunda ocorrência).
            let argumentos: Vec<CString> = opcoes_do_gerador().iter().map(|a| CString::new(a.as_str()).expect("opção sem NUL")).collect();
            let ponteiros: Vec<*const c_char> = argumentos.iter().map(|a| a.as_ptr()).collect();
            // SAFETY: `argv` aponta para strings C vivas até o fim da chamada.
            unsafe { LLVMParseCommandLineOptions(ponteiros.len() as i32, ponteiros.as_ptr(), c"dartforge".as_ptr()) };
            Ok(())
        })
        .clone()
}

/// As raízes de um statepoint (os operandos `"deopt"`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.12) podem ficar no
/// registrador preservado em que já estão (`use-registers-for-deopt-values`;
/// o mapa diz qual, com o local `Register`): sem isso, o padrão do LLVM,
/// cada uma é gravada num slot antes de toda chamada que coleta — o que a
/// pilha-sombra não paga. `DARTFORGE_MAPAS_REGISTRADORES=0` volta ao slot
/// (medida).
pub fn raizes_em_registrador() -> bool {
    static SIM: OnceLock<bool> = OnceLock::new();
    *SIM.get_or_init(|| std::env::var("DARTFORGE_MAPAS_REGISTRADORES").map_or(true, |v| v.trim() != "0"))
}

/// As opções do gerador de código que o dartforge passa ao LLVM (o
/// embutido, e o `ld64.lld` por `-mllvm`), sem o nome do programa.
pub fn opcoes_do_gerador_sem_programa() -> Vec<String> {
    vec![format!("-use-registers-for-deopt-values={}", raizes_em_registrador())]
}

fn opcoes_do_gerador() -> Vec<String> {
    let mut v = vec!["dartforge".to_owned()];
    v.extend(opcoes_do_gerador_sem_programa());
    v
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
    /// A CPU-alvo (o nome do LLVM, como o `-march` do Clang; `native` é a
    /// desta máquina, com os recursos dela); `None`, a padrão do Clang
    /// ([`cpu_padrao`]).
    pub cpu: Option<&'static str>,
}

impl Opcoes {
    /// O que entra na chave de um objeto em cache: dois módulos iguais com
    /// opções de mesma descrição dão os mesmos bytes.
    pub fn descricao(&self) -> String {
        let cpu = match self.cpu {
            None => String::new(),
            Some("native") => {
                let (nome, recursos) = cpu_da_maquina();
                format!(" cpu=native({nome};{recursos})")
            }
            Some(c) => format!(" cpu={c}"),
        };
        format!(
            "{} {}{cpu}",
            if self.otimizar { "O2" } else { "O0" },
            match self.formato {
                Formato::Objeto => "objeto",
                Formato::Bitcode => "bitcode-lto",
            }
        )
    }
}

/// O nome e os recursos da CPU desta máquina, como o LLVM os detecta (o
/// `-march=native` do Clang).
fn cpu_da_maquina() -> (String, String) {
    // SAFETY: as strings devolvidas são nossas e liberadas por
    // `tomar_mensagem`.
    unsafe { (tomar_mensagem(LLVMGetHostCPUName()), tomar_mensagem(LLVMGetHostCPUFeatures())) }
}

/// A identidade do gerador para as chaves de cache: a versão do LLVM, o
/// triple e a CPU do hospedeiro. Outro LLVM ou outra máquina-alvo não
/// reaproveitam objeto deste.
pub fn identidade() -> Result<String, String> {
    static ID: OnceLock<Result<String, String>> = OnceLock::new();
    ID.get_or_init(|| {
        inicializar_alvo_nativo()?;
        let triple = triple_padrao();
        Ok(format!("llvm-embutido {} {} {} {}", versao(), triple, cpu_padrao(&triple), opcoes_do_gerador_sem_programa().join(" ")))
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
    // Raízes por mapas de pilha (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    // §14.8): o módulo com funções `gc "statepoint-example"` passa pelo
    // `rewrite-statepoints-for-gc` — depois da otimização, para que o
    // inlining e o resto do pipeline vejam chamadas comuns — e pelo
    // verificador (o passe já produziu IR inválido que só morria na seleção
    // de instruções). O gerador de código dessas funções nunca é o do `-O0`
    // (o FastISel tem defeitos conhecidos com `gc.relocate`).
    let com_mapas = ir.contains(MARCA_DE_GC);
    let maquina = MaquinaAlvo::do_triple(&triple, opcoes.otimizar || com_mapas, opcoes.cpu)?;
    modulo.completar_alvo(&triple, &maquina, com_mapas);
    // O bitcode da produção nunca passa pelo passe dos mapas aqui: ele roda
    // depois de toda a otimização da ligação — no Windows e no Linux no fecho
    // do ThinLTO distribuído (`emit_native/src/lto_distribuida.rs`), no
    // Mach-O na LTO do `ld64.lld` (`--lto-newpm-passes`, `ligador_macos.rs`)
    // (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.4). Reescrito antes, o IR
    // seria otimizado de novo na LTO com os statepoints já postos, e o
    // otimizador não conhece as raízes deles: o executável perde raízes.
    let passe_no_ligador = opcoes.formato == Formato::Bitcode;
    // `DARTFORGE_PIPELINE_OBJETO` troca o pipeline do objeto otimizado
    // (medida de tempo e tamanho: `default<O1>`, `default<Os>`;
    // docs/NATIVO-PRODUCAO-GRANDE.md).
    let escolhido = std::env::var("DARTFORGE_PIPELINE_OBJETO").ok().filter(|p| !p.is_empty());
    let pipeline = match (opcoes.formato, opcoes.otimizar) {
        (Formato::Bitcode, _) => "lto-pre-link<O2>",
        (Formato::Objeto, true) => escolhido.as_deref().unwrap_or("default<O2>"),
        (Formato::Objeto, false) => "default<O0>",
    };
    if com_mapas && !passe_no_ligador {
        modulo.otimizar(pipeline, &maquina)?;
        modulo.tirar_estaticos_do_deopt();
        modulo.otimizar("rewrite-statepoints-for-gc,verify", &maquina)?;
        // O passe declara `@__tmp_use` para os usos provisórios que ele mesmo
        // apaga. A declaração que sobra não muda um objeto, mas no bitcode da
        // produção (a LTO do `lld-link`) ela entra na tabela de símbolos como
        // indefinida, e a ligação falha.
        modulo.remover_declaracao_sem_uso("__tmp_use");
        conferir_depois_do_rs4gc(nome, &modulo)?;
    } else {
        modulo.otimizar(pipeline, &maquina)?;
    }
    match opcoes.formato {
        Formato::Objeto => {
            despejar_antes_do_gerador(nome, &modulo);
            maquina.emitir_objeto(&modulo).map_err(|e| despejar_se_pedido(nome, "objeto", &modulo, e))
        }
        Formato::Bitcode => Ok(modulo.bitcode()),
    }
}

/// O valor `v` é um endereço fixo da imagem: um global, uma expressão
/// constante, ou a conversão, o `getelementptr` de índices constantes ou a
/// soma com constante de um deles; ou a carga de um dos objetos canônicos
/// `true`/`false` do contexto da thread (`getelementptr` sobre o
/// `dartforge_contexto()` em 360 ou 368, `layout::contexto`, direto ou pelo
/// `select` do `df.caixa_bool`), estáticos da imagem do runtime — a carga é
/// `!invariant.load`, e o gerador de código a rematerializa como uma
/// constante ([`Modulo::tirar_estaticos_do_deopt`]).
///
/// # Safety
/// `v` é um valor vivo de um módulo.
unsafe fn endereco_estatico(v: llvm_sys::prelude::LLVMValueRef, profundidade: u32) -> bool {
    // SAFETY: o contrato da função; só leituras.
    unsafe {
        if !LLVMIsAGlobalValue(v).is_null() || !LLVMIsAConstantExpr(v).is_null() {
            return true;
        }
        if profundidade > 4 || LLVMIsAInstruction(v).is_null() {
            return false;
        }
        let constante = |x| !LLVMIsAConstantInt(x).is_null();
        match LLVMGetInstructionOpcode(v) {
            LLVMOpcode::LLVMPtrToInt | LLVMOpcode::LLVMIntToPtr | LLVMOpcode::LLVMBitCast | LLVMOpcode::LLVMAddrSpaceCast => {
                endereco_estatico(LLVMGetOperand(v, 0), profundidade + 1)
            }
            LLVMOpcode::LLVMGetElementPtr => {
                (1..LLVMGetNumOperands(v) as u32).all(|k| constante(LLVMGetOperand(v, k)))
                    && endereco_estatico(LLVMGetOperand(v, 0), profundidade + 1)
            }
            LLVMOpcode::LLVMAdd => {
                let (a, b) = (LLVMGetOperand(v, 0), LLVMGetOperand(v, 1));
                (constante(b) && endereco_estatico(a, profundidade + 1)) || (constante(a) && endereco_estatico(b, profundidade + 1))
            }
            LLVMOpcode::LLVMLoad => carga_de_canonico(LLVMGetOperand(v, 0)),
            _ => false,
        }
    }
}

/// O valor inteiro de `v`, se ele é constante: um `ConstantInt` ou uma
/// operação inteira (soma, subtração, multiplicação, deslocamentos, `and`,
/// `or`, `xor`) de operandos constantes, com a aritmética de 64 bits do LLVM
/// ([`Modulo::tirar_estaticos_do_deopt`]).
///
/// # Safety
/// `v` é um valor vivo de um módulo.
unsafe fn valor_constante(v: llvm_sys::prelude::LLVMValueRef, profundidade: u32) -> Option<u64> {
    // SAFETY: o contrato da função; só leituras.
    unsafe {
        if !LLVMIsAConstantInt(v).is_null() {
            return Some(LLVMConstIntGetZExtValue(v));
        }
        if profundidade > 6 || LLVMIsAInstruction(v).is_null() || LLVMGetNumOperands(v) != 2 {
            return None;
        }
        let a = valor_constante(LLVMGetOperand(v, 0), profundidade + 1)?;
        let b = valor_constante(LLVMGetOperand(v, 1), profundidade + 1)?;
        Some(match LLVMGetInstructionOpcode(v) {
            LLVMOpcode::LLVMAdd => a.wrapping_add(b),
            LLVMOpcode::LLVMSub => a.wrapping_sub(b),
            LLVMOpcode::LLVMMul => a.wrapping_mul(b),
            LLVMOpcode::LLVMShl if b < 64 => a << b,
            LLVMOpcode::LLVMLShr if b < 64 => a >> b,
            LLVMOpcode::LLVMAShr if b < 64 => ((a as i64) >> b) as u64,
            LLVMOpcode::LLVMAnd => a & b,
            LLVMOpcode::LLVMOr => a | b,
            LLVMOpcode::LLVMXor => a ^ b,
            _ => return None,
        })
    }
}

/// O ponteiro `p` é o campo do `true` ou do `false` canônico no contexto:
/// `getelementptr` sobre a chamada a `dartforge_contexto` com o
/// deslocamento 360 ou 368, constante ou escolhido por um `select` entre os
/// dois.
///
/// # Safety
/// `p` é um valor vivo de um módulo.
unsafe fn carga_de_canonico(p: llvm_sys::prelude::LLVMValueRef) -> bool {
    const VERDADEIRO: u64 = 360;
    const FALSO: u64 = 368;
    // SAFETY: o contrato da função; só leituras.
    unsafe {
        if LLVMIsAInstruction(p).is_null() || LLVMGetInstructionOpcode(p) != LLVMOpcode::LLVMGetElementPtr || LLVMGetNumOperands(p) != 2 {
            return false;
        }
        let base = LLVMGetOperand(p, 0);
        if LLVMIsACallInst(base).is_null() {
            return false;
        }
        let chamado = LLVMGetCalledValue(base);
        let mut n = 0usize;
        let nome = LLVMGetValueName2(chamado, &mut n);
        if nome.is_null() || std::slice::from_raw_parts(nome.cast::<u8>(), n) != b"dartforge_contexto" {
            return false;
        }
        let canonico = |x| !LLVMIsAConstantInt(x).is_null() && matches!(LLVMConstIntGetZExtValue(x), VERDADEIRO | FALSO);
        let indice = LLVMGetOperand(p, 1);
        if canonico(indice) {
            return true;
        }
        !LLVMIsAInstruction(indice).is_null()
            && LLVMGetInstructionOpcode(indice) == LLVMOpcode::LLVMSelect
            && canonico(LLVMGetOperand(indice, 1))
            && canonico(LLVMGetOperand(indice, 2))
    }
}

/// O atributo das funções com raízes no mapa de pilha, como o emissor o
/// escreve (`emit_native/src/llvm/mod.rs`).
pub const MARCA_DE_GC: &str = "gc \"statepoint-example\"";


/// O objeto nativo de uma parte do **ThinLTO distribuído**
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.4 e Etapa 3): `bitcode` é o
/// módulo **depois** da importação e da otimização do backend do ThinLTO
/// (`clang -x ir parte.o -fthinlto-index=… -emit-llvm -c`). Aqui só entra o
/// que o `lld-link` não sabe rodar: o `rewrite-statepoints-for-gc`, no fim
/// de toda a otimização — o inlining entre o programa e o SDK já aconteceu
/// —, o verificador e a geração de código (nunca a do `-O0`).
///
/// Devolve o objeto e se o módulo tinha funções com raízes no mapa (o
/// objeto então traz o `.llvm_stackmaps`, que quem chama converte). A
/// pergunta é feita ao módulo lido: no bitcode o nome da estratégia de
/// coleta pode estar codificado, e procurá-lo nos bytes falha (o do Clang).
///
/// # Erros
/// Bitcode que o leitor recusa, falha do passe, do verificador ou do
/// gerador de código — a mensagem é a do LLVM.
pub fn gerar_de_bitcode(nome: &str, bitcode: &[u8], cpu: Option<&'static str>) -> Result<(Vec<u8>, bool), String> {
    inicializar_alvo_nativo()?;
    let contexto = Contexto::novo();
    let modulo = contexto.ler_bytes(nome, bitcode)?;
    let triple = modulo.triple().unwrap_or_else(triple_padrao);
    let maquina = MaquinaAlvo::do_triple(&triple, true, cpu)?;
    let com_mapas = modulo.tem_funcao_gc();
    modulo.completar_alvo(&triple, &maquina, com_mapas);
    if com_mapas {
        modulo.tirar_estaticos_do_deopt();
        modulo.otimizar("rewrite-statepoints-for-gc,verify", &maquina)?;
        modulo.remover_declaracao_sem_uso("__tmp_use");
        conferir_depois_do_rs4gc(nome, &modulo)?;
    } else {
        modulo.otimizar("verify", &maquina)?;
    }
    despejar_antes_do_gerador(nome, &modulo);
    Ok((maquina.emitir_objeto(&modulo).map_err(|e| despejar_se_pedido(nome, "objeto", &modulo, e))?, com_mapas))
}

/// `DARTFORGE_CONFERIR_RS4GC=1`: o conferidor das raízes depois do
/// `rewrite-statepoints-for-gc` ([`conferir_rs4gc`],
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.4) sobre o texto do módulo.
/// Caro (imprime o módulo inteiro): ligado nos testes do modo mapas.
fn conferir_depois_do_rs4gc(nome: &str, modulo: &Modulo<'_>) -> Result<(), String> {
    if !std::env::var("DARTFORGE_CONFERIR_RS4GC").is_ok_and(|v| v == "1") {
        return Ok(());
    }
    conferir_rs4gc::conferir(&modulo.texto()).map_err(|e| despejar_se_pedido(nome, "rs4gc", modulo, format!("{nome}: {e}")))
}

/// `DARTFORGE_DESPEJAR_MODULO=<trecho>`: o módulo cujo nome contém o trecho
/// vai para o diretório temporário logo antes do gerador de código
/// (`gerador-<nome>.ll`) — para a falha em que o LLVM derruba o processo
/// depois de imprimir o erro ("ran out of registers").
fn despejar_antes_do_gerador(nome: &str, modulo: &Modulo<'_>) {
    if let Ok(trecho) = std::env::var("DARTFORGE_DESPEJAR_MODULO")
        && !trecho.is_empty()
        && nome.contains(&trecho)
    {
        let nome_do_arquivo: String = nome.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        let destino = std::env::temp_dir().join(format!("gerador-{nome_do_arquivo}.ll"));
        let _ = std::fs::write(&destino, modulo.texto());
        eprintln!("dartforge: o módulo antes do gerador ficou em {}", destino.display());
    }
}

/// Com `DARTFORGE_KEEP_IR`, o módulo de uma falha (do conferidor, do gerador
/// de código) vai para o diretório temporário, `<etapa>-<nome>.ll`, para o
/// diagnóstico. Devolve o erro.
fn despejar_se_pedido(nome: &str, etapa: &str, modulo: &Modulo<'_>, erro: String) -> String {
    if std::env::var_os("DARTFORGE_KEEP_IR").is_some() {
        let nome_do_arquivo: String = nome.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        let destino = std::env::temp_dir().join(format!("{etapa}-{nome_do_arquivo}.ll"));
        let _ = std::fs::write(&destino, modulo.texto());
        eprintln!("dartforge: o módulo da falha ficou em {}", destino.display());
    }
    erro
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
        self.ler_bytes(nome, ir.as_bytes())
    }

    /// Lê o IR em texto ou em bitcode (o `LLVMParseIRInContext2` reconhece
    /// os dois pelo começo do buffer).
    fn ler_bytes(&self, nome: &str, ir: &[u8]) -> Result<Modulo<'_>, String> {
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
    /// Completa o triple e a camada de dados do módulo que não os escreve
    /// (o emissor os omite no macOS e no Linux aarch64). Com raízes por
    /// mapas (`com_mapas`), a camada de dados leva o `-ni:1`: o
    /// `addrspace(1)` das raízes é de ponteiros não integrais, e o otimizador
    /// não fabrica `ptrtoint`/`inttoptr` com elas (§14.8).
    fn completar_alvo(&self, triple: &str, maquina: &MaquinaAlvo, com_mapas: bool) {
        // SAFETY: o módulo e a máquina estão vivos; a camada de dados criada
        // é copiada para o módulo e liberada; o triple e o texto da camada de
        // dados são copiados pelo LLVM.
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
            if com_mapas {
                let atual = CStr::from_ptr(LLVMGetDataLayoutStr(self.m)).to_string_lossy().into_owned();
                if !atual.split('-').any(|p| p.starts_with("ni:"))
                    && let Ok(novo) = CString::new(format!("{atual}-ni:1"))
                {
                    LLVMSetDataLayout(self.m, novo.as_ptr());
                }
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

    /// Alguma função do módulo tem estratégia de coleta (`gc "…"`).
    fn tem_funcao_gc(&self) -> bool {
        // SAFETY: módulo vivo; a lista de funções é percorrida sem mudança.
        unsafe {
            let mut f = LLVMGetFirstFunction(self.m);
            while !f.is_null() {
                if !LLVMGetGC(f).is_null() {
                    return true;
                }
                f = LLVMGetNextFunction(f);
            }
        }
        false
    }

    /// Raízes por mapas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.12): o
    /// operando `"deopt"` que a otimização tornou um endereço estático — o
    /// handle de um objeto da imagem (`ptrtoint (gep @df.s…, 2)`), que chega
    /// pelo parâmetro de uma função copiada pelo inliner — vira nulo. O
    /// objeto da imagem nunca é coletado e não precisa de raiz; e, com as
    /// raízes em registrador, a constante teria de ser materializada num
    /// registrador na chamada, sem poder ir para a pilha (o alocador a
    /// rematerializa em vez de derramar): com várias, o gerador de código
    /// falha com "ran out of registers". Pelo mesmo motivo, o operando que é
    /// uma instrução de operandos todos constantes (o IR sem otimização do
    /// desenvolvimento: o `Smi` de um literal, `or (shl 3, 1), 1`) vira a
    /// constante dobrada, que vai ao mapa como `Constant`, sem registrador.
    /// Roda depois da otimização e antes do `rewrite-statepoints-for-gc`.
    /// Devolve quantos operandos trocou.
    fn tirar_estaticos_do_deopt(&self) -> usize {
        let mut trocados = 0;
        // SAFETY: módulo vivo; as instruções são percorridas sem mudar a
        // lista, e só um operando de bundle é trocado por um valor do mesmo
        // tipo. O bundle obtido é liberado logo depois de lido.
        unsafe {
            let mut f = LLVMGetFirstFunction(self.m);
            while !f.is_null() {
                let mut b = LLVMGetFirstBasicBlock(f);
                while !b.is_null() {
                    let mut i = LLVMGetFirstInstruction(b);
                    while !i.is_null() {
                        if !LLVMIsACallInst(i).is_null() || !LLVMIsAInvokeInst(i).is_null() {
                            let mut pos = LLVMGetNumArgOperands(i);
                            for k in 0..LLVMGetNumOperandBundles(i) {
                                let ob = LLVMGetOperandBundleAtIndex(i, k);
                                let mut n = 0usize;
                                let tag = LLVMGetOperandBundleTag(ob, &mut n);
                                let e_deopt = std::slice::from_raw_parts(tag.cast::<u8>(), n) == b"deopt";
                                let args = LLVMGetNumOperandBundleArgs(ob);
                                LLVMDisposeOperandBundle(ob);
                                if e_deopt {
                                    for j in pos..pos + args {
                                        let v = LLVMGetOperand(i, j);
                                        if endereco_estatico(v, 0) {
                                            LLVMSetOperand(i, j, LLVMConstNull(LLVMTypeOf(v)));
                                            trocados += 1;
                                        } else if !LLVMIsAInstruction(v).is_null()
                                            && let Some(c) = valor_constante(v, 0)
                                        {
                                            LLVMSetOperand(i, j, LLVMConstInt(LLVMTypeOf(v), c, 0));
                                            trocados += 1;
                                        }
                                    }
                                }
                                pos += args;
                            }
                        }
                        i = LLVMGetNextInstruction(i);
                    }
                    b = LLVMGetNextBasicBlock(b);
                }
                f = LLVMGetNextFunction(f);
            }
        }
        trocados
    }

    /// Apaga a declaração `nome`, se ela existir e não tiver uso.
    fn remover_declaracao_sem_uso(&self, nome: &str) {
        let Ok(c) = CString::new(nome) else { return };
        // SAFETY: módulo vivo; a função só é apagada sem usos.
        unsafe {
            let f = LLVMGetNamedFunction(self.m, c.as_ptr());
            if !f.is_null() && LLVMGetFirstUse(f).is_null() {
                LLVMDeleteFunction(f);
            }
        }
    }

    fn bitcode(&self) -> Vec<u8> {
        // SAFETY: o buffer criado é nosso e sai por `tomar_buffer`.
        unsafe { tomar_buffer(LLVMWriteBitcodeToMemoryBuffer(self.m)) }
    }

    /// O texto do módulo (o IR como o LLVM o imprime).
    fn texto(&self) -> String {
        // SAFETY: módulo vivo; a mensagem devolvida é nossa e liberada por
        // `tomar_mensagem`.
        unsafe { tomar_mensagem(LLVMPrintModuleToString(self.m)) }
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
    /// A máquina do `triple` com a CPU pedida, ou a padrão do Clang
    /// ([`cpu_padrao`]).
    /// Código independente de posição fora do Windows: os executáveis do
    /// Linux são PIE (o padrão do Clang) e as bibliotecas compartilhadas do
    /// SDK o exigem; o Mach-O é sempre PIC.
    fn do_triple(triple: &str, otimizar: bool, cpu: Option<&str>) -> Result<Self, String> {
        let triple_c = CString::new(triple).map_err(|_| format!("triple com NUL: {triple:?}"))?;
        // `native`: o nome e os recursos detectados (o que o Clang faz com
        // `-march=native`); outro nome, os recursos implícitos dele.
        let (cpu, recursos) = match cpu {
            Some("native") => cpu_da_maquina(),
            Some(c) => (c.to_owned(), String::new()),
            None => (cpu_padrao(triple).to_owned(), String::new()),
        };
        let cpu = CString::new(cpu).map_err(|_| "CPU com NUL".to_owned())?;
        let recursos = CString::new(recursos).map_err(|_| "recursos com NUL".to_owned())?;
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
                recursos.as_ptr(),
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
            let obj = gerar("soma", IR, &Opcoes { otimizar, formato: Formato::Objeto, cpu: None }).unwrap();
            let magico = &obj[..4];
            // ELF, Mach-O (64 bits) ou COFF x86-64/arm64.
            assert!(
                magico == b"\x7fELF" || magico == [0xcf, 0xfa, 0xed, 0xfe] || obj[..2] == [0x64, 0x86] || obj[..2] == [0x64, 0xaa],
                "cabeçalho inesperado: {magico:x?}"
            );
        }
        let bc = gerar("soma", IR, &Opcoes { otimizar: true, formato: Formato::Bitcode, cpu: None }).unwrap();
        assert_eq!(&bc[..4], b"BC\xc0\xde");
    }

    #[test]
    fn mesmo_ir_mesmos_bytes() {
        let op = Opcoes { otimizar: true, formato: Formato::Objeto, cpu: None };
        assert_eq!(gerar("a", IR, &op).unwrap(), gerar("a", IR, &op).unwrap());
    }

    #[test]
    fn ir_invalido_da_erro_com_a_mensagem_do_llvm() {
        let e = gerar("ruim", "define i32 @f( {", &Opcoes { otimizar: false, formato: Formato::Objeto, cpu: None }).unwrap_err();
        assert!(e.contains("ruim"), "{e}");
    }

    #[test]
    fn cpu_do_clang() {
        assert_eq!(cpu_padrao("x86_64-pc-windows-msvc"), "x86-64");
        assert_eq!(cpu_padrao("arm64-apple-darwin24.1.0"), "apple-m1");
        assert_eq!(cpu_padrao("aarch64-unknown-linux-gnu"), "generic");
    }
}
