//! Símbolos de runtime registrados no `JITModule` pelo endereço das funções Rust.
//!
//! O backend LLVM AOT liga o programa ao harness de `dartforge-runtime`, que
//! define `dartforge_print_i64(i64)` e `dartforge_print_bool(i8)` com `println!`.
//! O JIT não liga nada: registra estes mesmos nomes no `JITBuilder` apontando
//! para as funções Rust abaixo, que reproduzem exatamente aquele formato de
//! saída (`{valor}` e `true`/`false`, cada um seguido de `\n`).
//!
//! A saída é desviada por uma captura *por thread*. O código gerado executa na
//! mesma thread que chamou [`crate::ProgramaCompilado::executar_capturando`],
//! então a captura é suficiente para os testes e não sincroniza nada.
use std::cell::RefCell;

thread_local! {
    /// Buffer ativo da thread; `None` envia a saída para o stdout do processo.
    static CAPTURA: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Escreve uma linha no buffer da thread ou no stdout, como faz o harness AOT.
fn linha(texto: &str) {
    CAPTURA.with(|captura| {
        let mut captura = captura.borrow_mut();
        if let Some(buffer) = captura.as_mut() {
            buffer.push_str(texto);
            buffer.push('\n');
        } else {
            println!("{texto}");
        }
    });
}

/// Ativa a captura da thread e devolve o que estava ativo antes.
///
/// A restauração é responsabilidade de [`encerrar_captura`]; o par é usado
/// apenas por [`crate::ProgramaCompilado::executar_capturando`], que garante a
/// simetria mesmo quando o programa gerado retorna cedo.
pub(crate) fn iniciar_captura() -> Option<String> {
    CAPTURA.with(|captura| captura.borrow_mut().replace(String::new()))
}

/// Encerra a captura, devolve o texto acumulado e restaura o estado anterior.
pub(crate) fn encerrar_captura(anterior: Option<String>) -> String {
    CAPTURA.with(|captura| {
        std::mem::replace(&mut *captura.borrow_mut(), anterior).unwrap_or_default()
    })
}

/// Imprime um inteiro de 64 bits com sinal, igual a `dartforge_print_i64` do AOT.
extern "C" fn imprimir_i64(valor: i64) {
    linha(&valor.to_string());
}

/// Imprime o booleano codificado como 0 ou 1, igual a `dartforge_print_bool`.
///
/// O parâmetro é `u8` e não `bool` porque a ABI recebe o byte produzido por uma
/// comparação do código gerado; um `bool` Rust com outro padrão de bits seria
/// comportamento indefinido.
extern "C" fn imprimir_bool(valor: u8) {
    linha(if valor != 0 { "true" } else { "false" });
}

/// Nome e endereço de cada símbolo que o código gerado pode chamar.
///
/// A conversão passa por `usize` porque só a forma `ponteiro de função → usize →
/// ponteiro de dados` é um cast seguro; nenhum `unsafe` é necessário aqui.
pub(crate) fn simbolos() -> [(&'static str, *const u8); 2] {
    [
        (
            "dartforge_print_i64",
            imprimir_i64 as extern "C" fn(i64) as usize as *const u8,
        ),
        (
            "dartforge_print_bool",
            imprimir_bool as extern "C" fn(u8) as usize as *const u8,
        ),
    ]
}
