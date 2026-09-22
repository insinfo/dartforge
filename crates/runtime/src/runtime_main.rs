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

/// Imprime ponto flutuante de 64 bits (double).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_f64(value: f64) {
    if value.fract() == 0.0 && !value.is_infinite() && !value.is_nan() {
        println!("{value:.1}");
    } else {
        println!("{value}");
    }
}

/// Imprime qualquer objeto gerenciado pelo handle.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_handle(handle: i64) {
    if handle == 0 {
        println!("null");
        return;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        println!("{}", describe_handle(&heap, handle));
    });
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
#[unsafe(no_mangle)]
pub extern "C" fn main() -> i32 {
    // SAFETY: o objeto foi emitido para esta ABI e ligado pelo mesmo driver nativo.
    unsafe { dartforge_entry() };
    let pending = EXCEPTION.with(|slot| slot.borrow().is_some());
    if pending {
        let (bits, tag) = EXCEPTION.with(|slot| {
            let value = slot.borrow().expect("exceção verificada acima");
            untag(value)
        });
        HEAP.with(|heap| {
            let heap = heap.borrow();
            let detail = match tag {
                1 => format!("{bits}"),
                2 => format!("{}", bits != 0),
                _ => describe_handle(&heap, bits),
            };
            use std::io::Write;
            let _ = writeln!(
                std::io::stderr().lock(),
                "Uncaught exception: {detail}"
            );
        });
        std::process::exit(101);
    }
    if std::env::var("DARTFORGE_GC_STATS").as_deref() == Ok("1") {
        HEAP.with(|heap| {
            let s = heap.borrow().stats();
            eprintln!("{{\"dartforge_gc\":{{\"allocations\":{},\"collections\":{},\"reclaimed\":{},\"live_objects\":{},\"reserved_slots\":{},\"root_slots\":{},\"peak_root_slots\":{},\"live_roots\":{},\"peak_roots\":{},\"live_bytes\":{},\"peak_live_bytes\":{},\"permanent_roots\":{}}}}}",
                s.allocations, s.collections, s.reclaimed, s.live_objects, s.reserved_slots,
                s.root_slots, s.peak_root_slots, s.live_roots, s.peak_roots,
                s.estimated_bytes, s.peak_estimated_bytes, s.permanent_roots);
        });
    }
    0
}

use heap::{Heap, TaggedValue, Value};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static HEAP: RefCell<Heap> = RefCell::new(Heap::new(std::env::var_os("DARTFORGE_GC_STRESS").is_some()));
    static CLASS_NAMES: RefCell<HashMap<i64, String>> = RefCell::new(HashMap::new());
}

/// Registra o nome de uma classe pelo id para exibição em toString/print.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_class_name(class_id: i64, ptr: *const u8, len: i64) {
    let len = usize::try_from(len).expect("comprimento inválido");
    let bytes = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let name = std::str::from_utf8(bytes).expect("UTF-8").to_string();
    CLASS_NAMES.with(|map| map.borrow_mut().insert(class_id, name));
}

/// Exceção pendente do esquema portátil de `throw`/`try`/`catch`.
///
/// Em vez de desenrolamento nativo (`landingpad`/personalidade C++), que
/// exigiria alinhar o runtime Rust com o ABI de exceção do Clang em cada
/// plataforma, o emissor LLVM verifica `dartforge_exception_pending` após cada
/// chamada e desvia para o tratador. A carga é um valor com tag explícita:
/// 1 = int, 2 = bool, 3 = referência gerenciada viva, 4 = double. `throw null` é erro de
/// compilação no Dart 3.6.2 e nunca chega aqui.
thread_local! {
    static EXCEPTION: RefCell<Option<TaggedValue>> = RefCell::new(None);
}

/// Monta um valor com tag a partir da ABI plana (bits, tag); valida referências.
fn tagged(bits: i64, tag: u8) -> TaggedValue {
    match tag {
        1 => TaggedValue::scalar(bits),
        2 => TaggedValue::boolean(bits != 0),
        3 => TaggedValue::reference(bits),
        4 => TaggedValue {
            bits,
            is_ref: false,
            tag: heap::ValueTag::Double,
        },
        _ => panic!("tag de valor inválida"),
    }
}

/// Separa um valor na ABI plana (bits, tag) para chamadas LLVM.
fn untag(value: TaggedValue) -> (i64, u8) {
    use heap::ValueTag;
    let tag = match value.tag {
        ValueTag::Int => 1,
        ValueTag::Bool => 2,
        ValueTag::Ref => 3,
        ValueTag::Double => 4,
    };
    (value.bits, tag)
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
/// Obtém um valor enum canônico usando nome UTF-8 emitido como constante LLVM.
///
/// # Safety
/// O ponteiro deve identificar `len` bytes legíveis; class_id/index são IDs válidos.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_enum_get(class_id: i64, index: i64, ptr: *const u8, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let bytes = if len == 0 { &[] } else {
        // SAFETY: a constante LLVM permanece legível pelo comprimento informado.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let name = std::str::from_utf8(bytes).expect("UTF-8 inválido");
    HEAP.with(|heap| heap.borrow_mut().enum_value(class_id, index, name))
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

/// Compara igualdade (== de Dart) entre dois handles de referência.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_equal(a: i64, b: i64) -> u8 {
    if a == b { return 1; }
    if a == 0 || b == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        u8::from(heap.string_equal(a, b))
    })
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

/// Descreve um handle para mensagens de erro e impressão de coleções.
///
/// Profundidade limitada a 4 e 101 elementos, como a abreviação do SDK 3.6.2
/// para iteráveis; a forma exata é do subconjunto, não do SDK.
fn describe_handle(heap: &Heap, handle: i64) -> String {
    fn render(heap: &Heap, value: TaggedValue, depth: usize, output: &mut String) {
        if depth == 0 {
            output.push_str("...");
            return;
        }
        if value.is_ref {
            if value.bits == 0 {
                output.push_str("null");
                return;
            }
            match heap.get(value.bits) {
                Value::String(text) | Value::StringBuffer(text) => {
                    output.push_str(text);
                }
                Value::RawString(v) => {
                    output.push_str(&String::from_utf16_lossy(v));
                }
                Value::List(items) => {
                    let items: Vec<TaggedValue> = items.clone();
                    output.push('[');
                    for (index, item) in items.iter().take(101).enumerate() {
                        if index > 0 {
                            output.push_str(", ");
                        }
                        render(heap, *item, depth - 1, output);
                    }
                    if items.len() > 101 {
                        output.push_str(", ...");
                    }
                    output.push(']');
                }
                Value::Map(entries) => {
                    let entries: Vec<(TaggedValue, TaggedValue)> = entries.clone();
                    output.push('{');
                    for (index, (key, item)) in entries.iter().take(101).enumerate() {
                        if index > 0 {
                            output.push_str(", ");
                        }
                        render(heap, *key, depth - 1, output);
                        output.push_str(": ");
                        render(heap, *item, depth - 1, output);
                    }
                    if entries.len() > 101 {
                        output.push_str(", ...");
                    }
                    output.push('}');
                }
                Value::Set(items) => {
                    let items: Vec<TaggedValue> = items.clone();
                    output.push('{');
                    for (index, item) in items.iter().take(101).enumerate() {
                        if index > 0 {
                            output.push_str(", ");
                        }
                        render(heap, *item, depth - 1, output);
                    }
                    if items.len() > 101 {
                        output.push_str(", ...");
                    }
                    output.push('}');
                }
                Value::Closure { .. }
                | Value::Environment(_)
                | Value::Cell(_) => {
                    output.push_str("Instance");
                }
                Value::Record(items) => {
                    let items: Vec<TaggedValue> = items.clone();
                    output.push('(');
                    for (index, item) in items.iter().enumerate() {
                        if index > 0 {
                            output.push_str(", ");
                        }
                        render(heap, *item, depth - 1, output);
                    }
                    output.push(')');
                }
                Value::Object { class_id, .. } => {
                    let name = CLASS_NAMES.with(|map| map.borrow().get(class_id).cloned())
                        .unwrap_or_else(|| "Object".to_string());
                    output.push_str(&format!("Instance of '{name}'"));
                }
            }
            return;
        }
        use heap::ValueTag;
        match value.tag {
            ValueTag::Bool => output.push_str(if value.bits != 0 { "true" } else { "false" }),
            ValueTag::Int => output.push_str(&value.bits.to_string()),
            ValueTag::Double => {
                let d = f64::from_bits(value.bits as u64);
                if d.fract() == 0.0 && !d.is_infinite() && !d.is_nan() {
                    output.push_str(&format!("{d:.1}"));
                } else {
                    output.push_str(&d.to_string());
                }
            }
            ValueTag::Ref => {
                if value.bits != 0 {
                    render(heap, value, depth - 1, output);
                } else {
                    output.push_str("null");
                }
            }
        }
    }
    let mut output = String::new();
    render(
        heap,
        TaggedValue::reference(handle),
        4,
        &mut output,
    );
    output
}

/// Imprime uma lista gerenciada no formato `[1, 2]`, como `print` de Dart.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_list(handle: i64) {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        println!("{}", describe_handle(&heap, handle));
    });
}

/// Imprime um mapa gerenciado no formato `{1: a}`, como `print` de Dart.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_map(handle: i64) {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        println!("{}", describe_handle(&heap, handle));
    });
}

/// Imprime um conjunto gerenciado no formato `{1, 2}`, como `print` de Dart.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_set(handle: i64) {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        println!("{}", describe_handle(&heap, handle));
    });
}

/// Cria uma célula de captura mutável; o chamador a enraíza antes de coletar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_new(bits: i64, tag: u8) -> i64 {
    let value = tagged(bits, tag);
    HEAP.with(|heap| heap.borrow_mut().create_cell(value))
}

/// Lê os bits de uma captura mutável, sem copiar o objeto de uma referência.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_get_bits(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().cell_get(handle).bits)
}

/// Lê a tag (1 = int, 2 = bool, 3 = referência) de uma captura mutável.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_get_tag(handle: i64) -> u8 {
    HEAP.with(|heap| untag(heap.borrow().cell_get(handle)).1)
}

/// Atualiza a captura observada por todos os ambientes que partilham a célula.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_set(handle: i64, bits: i64, tag: u8) {
    let value = tagged(bits, tag);
    HEAP.with(|heap| heap.borrow_mut().cell_set(handle, value));
}

/// Cria um ambiente com `len` pares (bits, tag) lidos de `pairs`.
///
/// # Safety
/// `pairs` deve apontar para `2 * len` i64 legíveis; o emissor constrói o vetor
/// na pilha. Capturas mutáveis entram como handles de célula (tag 3).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_env_new(pairs: *const i64, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let captures = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: vetor temporário do emissor, legível pelos `2 * len` i64.
        let raw = unsafe { std::slice::from_raw_parts(pairs, len * 2) };
        raw.chunks_exact(2)
            .map(|pair| tagged(pair[0], u8::try_from(pair[1]).expect("tag inválida")))
            .collect()
    };
    HEAP.with(|heap| heap.borrow_mut().create_environment(captures))
}

/// Obtém a captura (handle de célula) por índice do ambiente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_env_get(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        heap.borrow()
            .environment_get(handle, usize::try_from(index).expect("índice inválido"))
            .bits
    })
}

/// Cria uma closure com identidade própria sobre código simbólico e ambiente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_new(code_id: i64, env: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().create_closure(code_id, env))
}

/// Devolve o tear-off canônico de uma função top-level (mesmo handle sempre).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_tearoff(code_id: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().tearoff(code_id))
}

/// Consulta o código simbólico de uma closure para despacho indireto.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_code(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().closure_parts(handle).0)
}

/// Consulta o ambiente de uma closure para chamadas indiretas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_env(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().closure_parts(handle).1)
}

/// Cria uma lista expansível com `len` pares (bits, tag) lidos de `pairs`.
///
/// # Safety
/// `pairs` deve apontar para `2 * len` i64 legíveis construídos pelo emissor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_list_new(pairs: *const i64, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let values = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: vetor temporário do emissor, legível pelos `2 * len` i64.
        let raw = unsafe { std::slice::from_raw_parts(pairs, len * 2) };
        raw.chunks_exact(2)
            .map(|pair| tagged(pair[0], u8::try_from(pair[1]).expect("tag inválida")))
            .collect()
    };
    HEAP.with(|heap| heap.borrow_mut().create_list(values))
}

/// Cria lista expansível vazia.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_new_empty() -> i64 {
    HEAP.with(|heap| heap.borrow_mut().create_list(Vec::new()))
}

/// Quantidade de elementos da lista.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_len(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().list_len(handle) as i64)
}

/// Lê os bits do elemento, sem verificar limites: o emissor verifica antes.
///
/// O índice fora dos limites lança `RangeError` capturável em vez de abortar;
/// o emissor desvia para o tratador ao observar a exceção pendente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_get_bits(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if index < 0 || index >= heap.list_len(handle) as i64 {
            drop(heap);
            let message = HEAP.with(|heap| {
                heap.borrow_mut().allocate(Value::String("RangeError".into()))
            });
            dartforge_exception_throw(message, 3);
            return 0;
        }
        heap.list_get(handle, index as usize).bits
    })
}

/// Lê a tag (1 = int, 2 = bool, 3 = referência) do elemento da lista.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_get_tag(handle: i64, index: i64) -> u8 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if index < 0 || index >= heap.list_len(handle) as i64 {
            return 0;
        }
        untag(heap.list_get(handle, index as usize)).1
    })
}

/// Substitui o elemento existente; limites verificados como na leitura.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_set(handle: i64, index: i64, bits: i64, tag: u8) {
    let value = tagged(bits, tag);
    HEAP.with(|heap| {
        if index < 0 || index >= heap.borrow().list_len(handle) as i64 {
            let message = heap
                .borrow_mut()
                .allocate(Value::String("RangeError".into()));
            dartforge_exception_throw(message, 3);
            return;
        }
        heap.borrow_mut().list_set(handle, index as usize, value);
    });
}

/// Acrescenta ao fim (`List.add`); devolve void pela ABI do emissor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_push(handle: i64, bits: i64, tag: u8) {
    let value = tagged(bits, tag);
    HEAP.with(|heap| heap.borrow_mut().list_push(handle, value));
}

/// Cria um mapa com `len` chaves e valores lidos de dois vetores de pares.
///
/// # Safety
/// `keys` e `values` apontam para `2 * len` i64 legíveis cada.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_map_new(keys: *const i64, values: *const i64, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let read = |ptr: *const i64| -> Vec<TaggedValue> {
        if len == 0 {
            Vec::new()
        } else {
            // SAFETY: vetores temporários do emissor, legíveis pelos `2 * len` i64.
            let raw = unsafe { std::slice::from_raw_parts(ptr, len * 2) };
            raw.chunks_exact(2)
                .map(|pair| tagged(pair[0], u8::try_from(pair[1]).expect("tag inválida")))
                .collect()
        }
    };
    let keys = read(keys);
    let values = read(values);
    HEAP.with(|heap| {
        heap.borrow_mut()
            .create_map(keys.into_iter().zip(values).collect())
    })
}

/// Quantidade de pares do mapa.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_len(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().map_len(handle) as i64)
}

/// Pertinência de chave (`containsKey`); nunca lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_contains(handle: i64, bits: i64, tag: u8) -> u8 {
    let key = tagged(bits, tag);
    HEAP.with(|heap| u8::from(heap.borrow().map_contains(handle, key)))
}

/// Lê os bits do valor; chave ausente devolve null (bits 0, tag verificada à parte).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_get_bits(handle: i64, bits: i64, tag: u8) -> i64 {
    let key = tagged(bits, tag);
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if heap.map_contains(handle, key) {
            heap.map_get(handle, key).bits
        } else {
            0
        }
    })
}

/// Lê a tag do valor; chave ausente devolve 0 (ausência, não null tipado).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_get_tag(handle: i64, bits: i64, tag: u8) -> u8 {
    let key = tagged(bits, tag);
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if heap.map_contains(handle, key) {
            untag(heap.map_get(handle, key)).1
        } else {
            0
        }
    })
}

/// Converte um valor (bits, tag) para uma string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_tagged_to_string(bits: i64, tag: u8) -> i64 {
    let value = tagged(bits, tag);
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        let mut out = String::new();
        match value.tag {
            heap::ValueTag::Int => out.push_str(&value.bits.to_string()),
            heap::ValueTag::Bool => out.push_str(if value.bits != 0 { "true" } else { "false" }),
            heap::ValueTag::Double => {
                let d = f64::from_bits(value.bits as u64);
                if d.fract() == 0.0 && !d.is_infinite() && !d.is_nan() {
                    out.push_str(&format!("{d:.1}"));
                } else {
                    out.push_str(&d.to_string());
                }
            }
            heap::ValueTag::Ref => {
                if value.bits != 0 {
                    if let Value::String(_) = heap_ref.get(value.bits) {
                        return value.bits;
                    } else {
                        out.push_str(&describe_handle(&heap_ref, value.bits));
                    }
                } else {
                    out.push_str("null");
                }
            }
        }
        drop(heap_ref);
        heap.borrow_mut().allocate(Value::String(out))
    })
}

/// Obtém valor do mapa já convertido para string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_get_to_string(handle: i64, bits: i64, tag: u8) -> i64 {
    let key = tagged(bits, tag);
    let val = HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if heap_ref.map_contains(handle, key) {
            heap_ref.map_get(handle, key)
        } else {
            tagged(0, 3) // null
        }
    });
    dartforge_tagged_to_string(val.bits, untag(val).1)
}

/// Insere ou substitui (`map[chave] = valor` e `[]=`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_set(
    handle: i64,
    key_bits: i64,
    key_tag: u8,
    value_bits: i64,
    value_tag: u8,
) {
    let key = tagged(key_bits, key_tag);
    let value = tagged(value_bits, value_tag);
    HEAP.with(|heap| heap.borrow_mut().map_set(handle, key, value));
}

/// Cria um conjunto com `len` pares (bits, tag); duplicadas conservam a primeira.
///
/// # Safety
/// `pairs` deve apontar para `2 * len` i64 legíveis construídos pelo emissor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_set_new(pairs: *const i64, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let values = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: vetor temporário do emissor, legível pelos `2 * len` i64.
        let raw = unsafe { std::slice::from_raw_parts(pairs, len * 2) };
        raw.chunks_exact(2)
            .map(|pair| tagged(pair[0], u8::try_from(pair[1]).expect("tag inválida")))
            .collect()
    };
    HEAP.with(|heap| heap.borrow_mut().create_set(values))
}

/// Quantidade de elementos distintos do conjunto.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_set_len(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().set_len(handle) as i64)
}

/// Pertinência (`contains`); nunca lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_set_contains(handle: i64, bits: i64, tag: u8) -> u8 {
    let value = tagged(bits, tag);
    HEAP.with(|heap| u8::from(heap.borrow().set_contains(handle, value)))
}

/// Insere quando ausente (`Set.add`); devolve se houve inserção.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_set_add(handle: i64, bits: i64, tag: u8) -> u8 {
    let value = tagged(bits, tag);
    HEAP.with(|heap| u8::from(heap.borrow_mut().set_add(handle, value)))
}

/// Consulta a classe nominal de um handle para testes `on T` de captura.
///
/// Devolve o `class_id` de objetos, -2 para strings, -3 para listas, -4 para
/// mapas, -5 para conjuntos e -6 para closures; outros valores internos nunca
/// são lançáveis pelo subconjunto e devolvem -1.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_value_class(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::Object { class_id, .. } => *class_id,
            Value::String(_) | Value::RawString(_) => -2,
            Value::StringBuffer(_) => -8,
            Value::List(_) => -3,
            Value::Map(_) => -4,
            Value::Set(_) => -5,
            Value::Closure { .. } => -6,
            Value::Record(_) => -7,
            Value::Cell(_) | Value::Environment(_) => -1,
        }
    })
}

/// Registra a exceção pendente; referências devem estar vivas e enraizadas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_throw(bits: i64, tag: u8) {
    let value = tagged(bits, tag);
    if value.is_ref && value.bits != 0 {
        HEAP.with(|heap| {
            heap.borrow().get(value.bits);
        });
    }
    EXCEPTION.with(|slot| *slot.borrow_mut() = Some(value));
}

/// Indica se há exceção pendente na thread corrente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_pending() -> u8 {
    EXCEPTION.with(|slot| u8::from(slot.borrow().is_some()))
}

/// Toma os bits da exceção pendente e limpa o slot.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_take_bits() -> i64 {
    EXCEPTION.with(|slot| slot.borrow_mut().take().map_or(0, |value| value.bits))
}

/// Lê a tag da exceção pendente sem limpar (a limpeza é de `take_bits`).
///
/// O emissor lê a tag antes dos bits: `take_bits` consome o slot e leituras
/// posteriores devolvem 0, que nenhuma carga válida usa.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_take_tag() -> u8 {
    EXCEPTION.with(|slot| slot.borrow().map_or(0, |value| untag(value).1))
}

/// Cria um novo Record no heap a partir de um array plano de pares (bits, tag).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_record_new(pairs: *const i64, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let items = if len == 0 {
        Vec::new()
    } else {
        let raw = unsafe { std::slice::from_raw_parts(pairs, len * 2) };
        raw.chunks_exact(2)
            .map(|chunk| tagged(chunk[0], chunk[1] as u8))
            .collect()
    };
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::Record(items)))
}

/// Converte um inteiro para string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_i64(value: i64) -> i64 {
    let s = value.to_string();
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s)))
}

/// Converte um double para string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_f64(value: f64) -> i64 {
    let s = if value.fract() == 0.0 && !value.is_infinite() && !value.is_nan() {
        format!("{value:.1}")
    } else {
        value.to_string()
    };
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s)))
}

/// Converte um booleano para string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_bool(value: u8) -> i64 {
    let s = if value != 0 { "true" } else { "false" }.to_string();
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s)))
}

/// Converte qualquer handle (String, Object, List, Map, Set, Record, null) para string.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_handle(handle: i64) -> i64 {
    if handle == 0 {
        let s = "null".to_string();
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s)));
    }
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if let Value::String(_) = heap_ref.get(handle) {
            return handle;
        }
        let text = describe_handle(&heap_ref, handle);
        drop(heap_ref);
        heap.borrow_mut().allocate(Value::String(text))
    })
}

/// Comprimento genérico para .length (String, List, Map, Set).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_generic_len(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.encode_utf16().count() as i64,
            Value::RawString(r) => r.len() as i64,
            Value::StringBuffer(b) => b.encode_utf16().count() as i64,
            Value::List(l) => l.len() as i64,
            Value::Map(m) => m.len() as i64,
            Value::Set(s) => s.len() as i64,
            _ => 0,
        }
    })
}

/// Comprimento em UTF-16 code units de uma string gerenciada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_len(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.encode_utf16().count() as i64,
            Value::RawString(r) => r.len() as i64,
            Value::StringBuffer(b) => b.encode_utf16().count() as i64,
            _ => 0,
        }
    })
}

/// Retorna o code unit UTF-16 no índice especificado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_code_unit_at(handle: i64, index: i64) -> i64 {
    if handle == 0 || index < 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.encode_utf16().nth(index as usize).unwrap_or(0) as i64,
            Value::RawString(r) => r.get(index as usize).copied().unwrap_or(0) as i64,
            Value::StringBuffer(b) => b.encode_utf16().nth(index as usize).unwrap_or(0) as i64,
            _ => 0,
        }
    })
}

/// Retorna uma lista de code units UTF-16 como inteiros.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_code_units(handle: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(Vec::new())));
    }
    let code_units: Vec<TaggedValue> = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.encode_utf16().map(|u| TaggedValue { bits: u as i64, tag: heap::ValueTag::Int, is_ref: false }).collect(),
            Value::RawString(r) => r.iter().map(|&u| TaggedValue { bits: u as i64, tag: heap::ValueTag::Int, is_ref: false }).collect(),
            Value::StringBuffer(b) => b.encode_utf16().map(|u| TaggedValue { bits: u as i64, tag: heap::ValueTag::Int, is_ref: false }).collect(),
            _ => Vec::new(),
        }
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(code_units)))
}

/// Retorna uma lista de runes (Unicode scalar values / code points) como inteiros.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_runes(handle: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(Vec::new())));
    }
    let runes: Vec<TaggedValue> = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.chars().map(|c| TaggedValue { bits: c as u32 as i64, tag: heap::ValueTag::Int, is_ref: false }).collect(),
            Value::RawString(r) => char::decode_utf16(r.iter().copied()).map(|res| TaggedValue { bits: res.unwrap_or('\u{FFFD}') as u32 as i64, tag: heap::ValueTag::Int, is_ref: false }).collect(),
            Value::StringBuffer(b) => b.chars().map(|c| TaggedValue { bits: c as u32 as i64, tag: heap::ValueTag::Int, is_ref: false }).collect(),
            _ => Vec::new(),
        }
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(runes)))
}

/// Converte um inteiro para string na base indicada (ex.: base 16 para hex minúsculo).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_int_to_radix_string(value: i64, radix: i64) -> i64 {
    let radix = radix.clamp(2, 36) as u32;
    let s = if value == 0 {
        "0".to_string()
    } else {
        let neg = value < 0;
        let mut uval = if neg { (-(value as i128)) as u64 } else { value as u64 };
        let mut digits = Vec::new();
        let uradix = radix as u64;
        while uval > 0 {
            let rem = (uval % uradix) as u32;
            let c = if rem < 10 { (b'0' + rem as u8) as char } else { (b'a' + (rem - 10) as u8) as char };
            digits.push(c);
            uval /= uradix;
        }
        if neg { digits.push('-'); }
        digits.into_iter().rev().collect()
    };
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s)))
}

/// Retorna substring usando índices UTF-16.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_substring(handle: i64, start: i64, end: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(String::new())));
    }
    let units: Vec<u16> = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            Value::StringBuffer(b) => b.encode_utf16().collect(),
            _ => Vec::new(),
        }
    });
    let len = units.len();
    let start_idx = (start.max(0) as usize).min(len);
    let end_idx = if end < 0 { len } else { (end as usize).min(len).max(start_idx) };
    let slice = &units[start_idx..end_idx];
    match String::from_utf16(slice) {
        Ok(s) => HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s))),
        Err(_) => HEAP.with(|heap| heap.borrow_mut().allocate(Value::RawString(slice.to_vec()))),
    }
}

/// Cria string a partir de um code point ou code unit UTF-16.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_from_char_code(code: i64) -> i64 {
    if (0xD800..=0xDFFF).contains(&code) {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::RawString(vec![code as u16])));
    }
    let s = if let Some(c) = char::from_u32(code as u32) {
        c.to_string()
    } else {
        String::new()
    };
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s)))
}

/// Cria string a partir de lista/iterável de char codes.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_from_char_codes(list_handle: i64) -> i64 {
    if list_handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(String::new())));
    }
    let units: Vec<u16> = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::List(items) = heap.get(list_handle) else { return Vec::new(); };
        let mut out = Vec::new();
        for item in items {
            let code = item.bits;
            if code <= 0xFFFF {
                out.push(code as u16);
            } else if let Some(c) = char::from_u32(code as u32) {
                let mut buf = [0u16; 2];
                out.extend_from_slice(c.encode_utf16(&mut buf));
            }
        }
        out
    });
    match String::from_utf16(&units) {
        Ok(s) => HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s))),
        Err(_) => HEAP.with(|heap| heap.borrow_mut().allocate(Value::RawString(units))),
    }
}

/// Localiza substring a partir do offset `start` (em UTF-16). Devolve -1 se não encontrar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_index_of(handle: i64, pat_handle: i64, start: i64) -> i64 {
    if handle == 0 || pat_handle == 0 { return -1; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            Value::StringBuffer(b) => b.encode_utf16().collect(),
            _ => return -1,
        };
        let pat_units: Vec<u16> = match heap.get(pat_handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return -1,
        };
        if pat_units.is_empty() { return 0; }
        let start_pos = (start.max(0) as usize).min(target_units.len());
        for i in start_pos..=target_units.len().saturating_sub(pat_units.len()) {
            if target_units[i..i + pat_units.len()] == pat_units[..] {
                return i as i64;
            }
        }
        -1
    })
}

/// Localiza última ocorrência de substring. Devolve -1 se não encontrar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_last_index_of(handle: i64, pat_handle: i64) -> i64 {
    if handle == 0 || pat_handle == 0 { return -1; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            Value::StringBuffer(b) => b.encode_utf16().collect(),
            _ => return -1,
        };
        let pat_units: Vec<u16> = match heap.get(pat_handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return -1,
        };
        if pat_units.is_empty() { return target_units.len() as i64; }
        if target_units.len() < pat_units.len() { return -1; }
        for i in (0..=target_units.len() - pat_units.len()).rev() {
            if target_units[i..i + pat_units.len()] == pat_units[..] {
                return i as i64;
            }
        }
        -1
    })
}

/// Divide a string pelo separador (split).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_split(handle: i64, pat_handle: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(Vec::new())));
    }
    let res_items: Vec<TaggedValue> = HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        let pat_str = if pat_handle != 0 {
            if let Value::String(s) = heap_ref.get(pat_handle) { s.as_str() } else { "" }
        } else { "" };
        let mut pieces = Vec::new();
        match heap_ref.get(handle) {
            Value::String(s) => {
                if pat_str.is_empty() {
                    for u in s.encode_utf16() {
                        let sub = match String::from_utf16(&[u]) {
                            Ok(v) => Value::String(v),
                            Err(_) => Value::RawString(vec![u]),
                        };
                        pieces.push(sub);
                    }
                } else {
                    for part in s.split(pat_str) {
                        pieces.push(Value::String(part.to_string()));
                    }
                }
            }
            Value::RawString(r) => {
                for &u in r {
                    pieces.push(Value::RawString(vec![u]));
                }
            }
            _ => {}
        }
        drop(heap_ref);
        let mut list = Vec::new();
        for val in pieces {
            let h = heap.borrow_mut().allocate(val);
            list.push(TaggedValue { bits: h, tag: heap::ValueTag::Ref, is_ref: true });
        }
        list
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(res_items)))
}

/// Verifica se a string contém a substring.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_contains(handle: i64, pat_handle: i64) -> u8 {
    (dartforge_string_index_of(handle, pat_handle, 0) >= 0) as u8
}

/// Substitui todas as ocorrências de `from` por `to`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_replace_all(handle: i64, from_handle: i64, to_handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let res_str = HEAP.with(|heap| {
        let heap = heap.borrow();
        let s = match heap.get(handle) {
            Value::String(s) => s.as_str(),
            _ => return String::new(),
        };
        let from = if from_handle != 0 {
            if let Value::String(f) = heap.get(from_handle) { f.as_str() } else { "" }
        } else { "" };
        let to = if to_handle != 0 {
            if let Value::String(t) = heap.get(to_handle) { t.as_str() } else { "" }
        } else { "" };
        s.replace(from, to)
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(res_str)))
}

/// Preenche à esquerda até a largura indicada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_pad_left(handle: i64, width: i64, pad_handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let pad_char = HEAP.with(|heap| {
        let heap = heap.borrow();
        if pad_handle != 0 {
            if let Value::String(p) = heap.get(pad_handle) { p.chars().next().unwrap_or(' ') } else { ' ' }
        } else { ' ' }
    });
    let s_len = dartforge_generic_len(handle);
    if width <= s_len { return handle; }
    let count = (width - s_len) as usize;
    let pad_str: String = std::iter::repeat(pad_char).take(count).collect();
    let pad_handle = HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(pad_str)));
    HEAP.with(|heap| heap.borrow_mut().string_concat(pad_handle, handle))
}

/// Retorna elementos da lista na ordem inversa.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_reversed(handle: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(Vec::new())));
    }
    let rev_items: Vec<TaggedValue> = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::List(items) = heap.get(handle) else { return Vec::new(); };
        items.iter().rev().copied().collect()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(rev_items)))
}

/// Aloca um novo StringBuffer vazio.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_buffer_new() -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::StringBuffer(String::new())))
}

/// Escreve no StringBuffer.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_buffer_write(buf_handle: i64, str_handle: i64) {
    if buf_handle == 0 || str_handle == 0 { return; }
    HEAP.with(|heap| {
        let mut heap_ref = heap.borrow_mut();
        let text_to_append = match heap_ref.get(str_handle) {
            Value::String(s) => s.clone(),
            Value::RawString(r) => String::from_utf16_lossy(r),
            _ => describe_handle(&heap_ref, str_handle),
        };
        if let Value::StringBuffer(buf) = heap_ref.get_mut(buf_handle) {
            buf.push_str(&text_to_append);
        }
    });
}



/// Retorna uma nova string convertida para maiúsculas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_to_upper(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.to_uppercase()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(text)))
}

/// Repete uma string `times` vezes (`'a' * 3`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_repeat(handle: i64, times: i64) -> i64 {
    if handle == 0 || times <= 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(String::new())));
    }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.repeat(times as usize)
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(text)))
}

/// Concatena os elementos de uma lista usando um separador em string.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_join(handle: i64, sep_handle: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(String::new())));
    }
    let joined = HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        let Value::List(items) = heap_ref.get(handle) else { return String::new(); };
        let sep = if sep_handle != 0 {
            if let Value::String(s) = heap_ref.get(sep_handle) { s.as_str() } else { "" }
        } else { "" };
        let mut parts = Vec::new();
        for item in items {
            let mut out = String::new();
            match item.tag {
                heap::ValueTag::Int => out.push_str(&item.bits.to_string()),
                heap::ValueTag::Bool => out.push_str(if item.bits != 0 { "true" } else { "false" }),
                heap::ValueTag::Double => {
                    let d = f64::from_bits(item.bits as u64);
                    if d.fract() == 0.0 && !d.is_infinite() && !d.is_nan() {
                        out.push_str(&format!("{d:.1}"));
                    } else {
                        out.push_str(&d.to_string());
                    }
                }
                heap::ValueTag::Ref => {
                    if item.bits != 0 {
                        if let Value::String(s) = heap_ref.get(item.bits) {
                            out.push_str(s);
                        } else {
                            out.push_str(&describe_handle(&heap_ref, item.bits));
                        }
                    } else {
                        out.push_str("null");
                    }
                }
            }
            parts.push(out);
        }
        parts.join(sep)
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(joined)))
}
