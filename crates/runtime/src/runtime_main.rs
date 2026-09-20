//! Harness standalone: ABI C restrita, sem ponteiros nem gerenciamento de memória.

/// Imprime um inteiro assinado recebido do módulo LLVM.
// SAFETY: o nome é reservado pelo compilador e há exatamente uma definição.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_i64(value: i64) {
    println!("{value}");
}

/// Imprime o booleano representado pelo byte 0 ou 1 no contrato LLVM.
// SAFETY: símbolo reservado; u8 evita passar um bool Rust inválido pela ABI.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_bool(value: u8) {
    println!("{}", value != 0);
}

/// Imprime o valor null sem expor uma representação de objeto pela ABI.
// SAFETY: símbolo reservado, definido exatamente uma vez no runtime.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_null() {
    println!("null");
}

/// Encerra o processo quando uma asserção de não nulidade falha.
///
/// Ainda não há exceções Dart capturáveis; a falha é explícita e não retorna.
// SAFETY: símbolo reservado e contrato C sem retorno, conforme a declaração LLVM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_null_assert_fail() -> ! {
    use std::io::Write;
    let _ = writeln!(std::io::stderr().lock(), "Null check operator used on a null value");
    std::process::exit(101)
}

// SAFETY: o emissor define esta entrada com assinatura C void(void).
unsafe extern "C" {
    fn dartforge_entry();
}

/// Invoca uma vez o programa ligado ao runtime Rust.
fn main() {
    // SAFETY: o objeto foi emitido para esta ABI e ligado pelo mesmo driver nativo.
    unsafe { dartforge_entry() };
}
