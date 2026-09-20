// Harness standalone: handles gerenciados e ABI C com raízes explícitas.

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
    let _ = writeln!(
        std::io::stderr().lock(),
        "Null check operator used on a null value"
    );
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
    if std::env::var("DARTFORGE_GC_STATS").as_deref() == Ok("1") {
        HEAP.with(|heap| {
            let s = heap.borrow().stats();
            eprintln!("{{\"dartforge_gc\":{{\"allocations\":{},\"collections\":{},\"reclaimed\":{},\"live_objects\":{},\"reserved_slots\":{},\"root_slots\":{},\"peak_root_slots\":{},\"live_roots\":{},\"peak_roots\":{},\"live_bytes\":{},\"peak_live_bytes\":{}}}}}",
                s.allocations, s.collections, s.reclaimed, s.live_objects, s.reserved_slots,
                s.root_slots, s.peak_root_slots, s.live_roots, s.peak_roots,
                s.estimated_bytes, s.peak_estimated_bytes);
        });
    }
}

use heap::{Heap, Value};
use std::cell::RefCell;
thread_local! {
    static HEAP: RefCell<Heap> = RefCell::new(Heap::new(std::env::var_os("DARTFORGE_GC_STRESS").is_some()));
}

/// Abre frame para raízes precisas dos valores SSA da função.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_push_frame(slot_count: i64) -> i64 {
    HEAP.with(|heap| {
        heap.borrow_mut().push_frame_with_slots(
            usize::try_from(slot_count).expect("quantidade de slots inválida"),
        )
    })
}
/// Substitui uma raiz estática; zero limpa o slot sem alterar o tamanho do frame.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_set_root(frame: i64, slot: i64, handle: i64) {
    HEAP.with(|heap| {
        heap.borrow_mut()
            .set_root(frame, usize::try_from(slot).expect("slot inválido"), handle)
    });
}
/// Protege handle positivo; zero representa null.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_root(frame: i64, handle: i64) {
    HEAP.with(|heap| heap.borrow_mut().root(frame, handle));
}
/// Remove raízes do frame sem disparar coleta durante retorno ao chamador.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_pop_frame(frame: i64) {
    HEAP.with(|heap| heap.borrow_mut().pop_frame(frame));
}
/// Permite coleta explícita em testes e futuras rotinas de manutenção.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_collect() {
    HEAP.with(|heap| heap.borrow_mut().collect());
}
/// Aloca objeto inicialmente zerado, com campos ainda sem referências.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_new(class_id: i64, field_count: i64) -> i64 {
    HEAP.with(|heap| {
        heap.borrow_mut().allocate(Value::Object {
            class_id,
            fields: vec![(0, false); usize::try_from(field_count).expect("campos inválidos")],
        })
    })
}
/// Obtém bits do campo pelo índice estável escolhido pelo emissor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_get(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::Object { fields, .. } = heap.get(handle) else {
            panic!("objeto esperado")
        };
        fields[usize::try_from(index).expect("índice inválido")].0
    })
}
/// Grava campo e informa explicitamente se seus bits são referência gerenciada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_set(handle: i64, index: i64, bits: i64, is_ref: u8) {
    HEAP.with(|heap| heap.borrow_mut().set(handle, index, bits, is_ref != 0));
}
/// Consulta identidade nominal para despacho virtual gerado pelo LLVM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_class(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::Object { class_id, .. } = heap.get(handle) else {
            panic!("objeto esperado")
        };
        *class_id
    })
}
/// Copia UTF-8 de uma constante LLVM para uma string gerenciada.
/// SAFETY: ptr deve apontar para len bytes legíveis; o emissor garante essa região.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_string_new(ptr: *const u8, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let bytes = if len == 0 {
        &[]
    } else {
        // SAFETY: única leitura de ponteiro estrangeiro; contrato da constante LLVM.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let text = std::str::from_utf8(bytes)
        .expect("UTF-8 inválido")
        .to_owned();
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(text)))
}
/// Concatena strings não nulas; argumentos devem estar enraizados pelo emissor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_concat(a: i64, b: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().string_concat(a, b))
}
/// Compara conteúdo UTF-8; dois handles null são iguais.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_equal(a: i64, b: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().string_equal(a, b)))
}
/// Imprime conteúdo da string gerenciada ou null para handle zero.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_string(handle: i64) {
    if handle == 0 {
        println!("null");
        return;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(text) = heap.get(handle) else {
            panic!("string esperada")
        };
        println!("{text}");
    });
}
