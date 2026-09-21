//! Implementação segura da ABI de runtime que o código JIT chama.
//!
//! O perfil AOT (`crates/native`) compila `dartforge_runtime::RUNTIME_MAIN` com
//! `rustc` e liga o objeto LLVM contra os símbolos `#[unsafe(no_mangle)]` desse
//! harness. O perfil JIT não liga nada: o código gerado em memória precisa
//! encontrar os mesmos nomes, então este módulo reimplementa exatamente o mesmo
//! contrato sobre o mesmo `dartforge_runtime::heap::Heap`, e [`crate::ffi`]
//! registra os endereços dessas funções como símbolos absolutos na `JITDylib`.
//!
//! Duplicar o corpo das funções seria fácil de divergir do AOT, então a regra é:
//! **toda mudança em `crates/runtime/src/runtime_main.rs` tem contrapartida
//! aqui**, e o teste diferencial JIT×AOT em `tests/execucao.rs` é o que prova
//! que os dois perfis continuam de acordo.
//!
//! Aqui não há `unsafe`: as funções recebem inteiros e fatias já validadas. A
//! conversão de ponteiro C para fatia acontece uma única vez, em [`crate::ffi`].
use dartforge_runtime::heap::{Heap, Value};
use std::cell::RefCell;

thread_local! {
    /// Heap gerenciado da thread que executa o código JIT.
    ///
    /// É `thread_local` pelo mesmo motivo do harness AOT: os handles não são
    /// ponteiros e o coletor enxerga apenas as raízes empilhadas por esta
    /// thread. Executar o mesmo módulo em outra thread veria um heap vazio.
    static HEAP: RefCell<Heap> =
        RefCell::new(Heap::new(std::env::var_os("DARTFORGE_GC_STRESS").is_some()));
    /// Destino das linhas impressas: `None` escreve em stdout, como o AOT.
    ///
    /// A captura existe porque o teste diferencial precisa comparar a saída do
    /// JIT (em processo) com a do executável AOT (em outro processo). Sem ela a
    /// comparação dependeria de redirecionar o stdout do próprio processo de
    /// teste, o que não é confiável em execução paralela.
    static SINK: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Restaura o destino anterior de impressão mesmo se a execução for interrompida.
struct SinkGuard(Option<String>);
impl Drop for SinkGuard {
    /// Devolve ao `SINK` o valor que existia antes da captura.
    fn drop(&mut self) {
        SINK.with(|sink| *sink.borrow_mut() = self.0.take());
    }
}

/// Executa `body` com a impressão desviada para um buffer e devolve o texto.
///
/// As linhas usam `\n` em qualquer sistema, como `println!`. O buffer anterior é
/// restaurado mesmo quando `body` retorna erro, e capturas podem ser aninhadas.
pub(crate) fn capturing<T>(body: impl FnOnce() -> T) -> (T, String) {
    let previous = SINK.with(|sink| sink.borrow_mut().replace(String::new()));
    let guard = SinkGuard(previous);
    let value = body();
    let captured = SINK
        .with(|sink| sink.borrow_mut().replace(String::new()))
        .unwrap_or_default();
    drop(guard);
    (value, captured)
}

/// Escreve uma linha no destino ativo, com a mesma quebra de `println!`.
fn line(text: &str) {
    let captured = SINK.with(|sink| {
        let mut sink = sink.borrow_mut();
        if let Some(buffer) = sink.as_mut() {
            buffer.push_str(text);
            buffer.push('\n');
            true
        } else {
            false
        }
    });
    if !captured {
        println!("{text}");
    }
}

/// Imprime um inteiro assinado recebido do módulo LLVM.
pub(crate) fn print_i64(value: i64) {
    line(&value.to_string());
}
/// Imprime o booleano representado pelo byte 0 ou 1 no contrato LLVM.
pub(crate) fn print_bool(value: u8) {
    line(if value != 0 { "true" } else { "false" });
}
/// Imprime o valor null sem expor uma representação de objeto pela ABI.
pub(crate) fn print_null() {
    line("null");
}
/// Encerra o processo quando uma asserção de não nulidade falha.
///
/// Repete o comportamento do harness AOT: não há exceções Dart capturáveis, a
/// falha é explícita e não retorna. No JIT isso derruba o **processo
/// hospedeiro**, porque o código gerado executa dentro dele; está documentado
/// como limite em `docs/JIT.md`.
pub(crate) fn null_assert_fail() -> ! {
    use std::io::Write;
    let _ = writeln!(
        std::io::stderr().lock(),
        "Null check operator used on a null value"
    );
    std::process::exit(101)
}
/// Abre frame para raízes precisas dos valores SSA da função.
pub(crate) fn gc_push_frame(slot_count: i64) -> i64 {
    HEAP.with(|heap| {
        heap.borrow_mut().push_frame_with_slots(
            usize::try_from(slot_count).expect("quantidade de slots inválida"),
        )
    })
}
/// Substitui uma raiz estática; zero limpa o slot sem alterar o tamanho do frame.
pub(crate) fn gc_set_root(frame: i64, slot: i64, handle: i64) {
    HEAP.with(|heap| {
        heap.borrow_mut()
            .set_root(frame, usize::try_from(slot).expect("slot inválido"), handle)
    });
}
/// Protege handle positivo; zero representa null.
pub(crate) fn gc_root(frame: i64, handle: i64) {
    HEAP.with(|heap| heap.borrow_mut().root(frame, handle));
}
/// Remove raízes do frame sem disparar coleta durante retorno ao chamador.
pub(crate) fn gc_pop_frame(frame: i64) {
    HEAP.with(|heap| heap.borrow_mut().pop_frame(frame));
}
/// Permite coleta explícita em testes e futuras rotinas de manutenção.
pub(crate) fn gc_collect() {
    HEAP.with(|heap| heap.borrow_mut().collect());
}
/// Aloca objeto inicialmente zerado, com campos ainda sem referências.
pub(crate) fn object_new(class_id: i64, field_count: i64) -> i64 {
    HEAP.with(|heap| {
        heap.borrow_mut().allocate(Value::Object {
            class_id,
            fields: vec![(0, false); usize::try_from(field_count).expect("campos inválidos")],
        })
    })
}
/// Obtém bits do campo pelo índice estável escolhido pelo emissor.
pub(crate) fn object_get(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::Object { fields, .. } = heap.get(handle) else {
            panic!("objeto esperado")
        };
        fields[usize::try_from(index).expect("índice inválido")].0
    })
}
/// Grava campo e informa explicitamente se seus bits são referência gerenciada.
pub(crate) fn object_set(handle: i64, index: i64, bits: i64, is_ref: u8) {
    HEAP.with(|heap| heap.borrow_mut().set(handle, index, bits, is_ref != 0));
}
/// Consulta identidade nominal para despacho virtual gerado pelo LLVM.
pub(crate) fn object_class(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::Object { class_id, .. } = heap.get(handle) else {
            panic!("objeto esperado")
        };
        *class_id
    })
}
/// Copia UTF-8 de uma constante LLVM já convertida em fatia para uma string gerenciada.
pub(crate) fn string_new(bytes: &[u8]) -> i64 {
    let text = std::str::from_utf8(bytes)
        .expect("UTF-8 inválido")
        .to_owned();
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(text)))
}
/// Obtém um valor enum canônico usando o nome UTF-8 emitido como constante LLVM.
pub(crate) fn enum_get(class_id: i64, index: i64, bytes: &[u8]) -> i64 {
    let name = std::str::from_utf8(bytes).expect("UTF-8 inválido");
    HEAP.with(|heap| heap.borrow_mut().enum_value(class_id, index, name))
}
/// Concatena strings não nulas; argumentos devem estar enraizados pelo emissor.
pub(crate) fn string_concat(a: i64, b: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().string_concat(a, b))
}
/// Compara conteúdo UTF-8; dois handles null são iguais.
pub(crate) fn string_equal(a: i64, b: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().string_equal(a, b)))
}
/// Imprime conteúdo da string gerenciada ou null para handle zero.
pub(crate) fn print_string(handle: i64) {
    if handle == 0 {
        line("null");
        return;
    }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(text) = heap.get(handle) else {
            panic!("string esperada")
        };
        text.clone()
    });
    line(&text);
}

/// Estatísticas do coletor da thread atual, para o equivalente de `DARTFORGE_GC_STATS`.
pub(crate) fn gc_stats() -> dartforge_runtime::heap::HeapStats {
    HEAP.with(|heap| heap.borrow().stats())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A captura desvia a impressão e devolve o destino anterior intacto.
    #[test]
    fn capture_redirects_and_restores() {
        let ((), text) = capturing(|| {
            print_i64(-42);
            print_bool(1);
            print_bool(0);
            print_null();
        });
        assert_eq!(text, "-42\ntrue\nfalse\nnull\n");
        let ((), outer) = capturing(|| {
            print_i64(1);
            let ((), inner) = capturing(|| print_i64(2));
            assert_eq!(inner, "2\n");
            print_i64(3);
        });
        assert_eq!(outer, "1\n3\n");
    }
    /// Strings gerenciadas seguem o mesmo contrato de handles do harness AOT.
    #[test]
    fn managed_strings_match_the_aot_contract() {
        let (handle, text) = capturing(|| {
            let frame = gc_push_frame(0);
            let a = string_new(b"ol");
            gc_root(frame, a);
            let b = string_new(b"a");
            gc_root(frame, b);
            let joined = string_concat(a, b);
            gc_root(frame, joined);
            print_string(joined);
            print_string(0);
            assert_eq!(string_equal(joined, joined), 1);
            gc_pop_frame(frame);
            joined
        });
        assert!(handle > 0);
        assert_eq!(text, "ola\nnull\n");
    }
}
