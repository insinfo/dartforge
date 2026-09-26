// Runtime nativo, núcleo: entrada `main`, estado por thread (heap, nomes e
// subtipos de classe), objetos, igualdade, caixas e records. Os fragmentos
// `crates/runtime/src/*.rs` listados em `FRAGMENTOS` (`crates/runtime/build.rs`)
// são concatenados num único programa Rust; este é o primeiro.

// Harness standalone: handles gerenciados e ABI C com raízes explícitas.

// SAFETY: o emissor define esta entrada com assinatura C void(void).
//
// Só no executável AOT. Quando este arquivo é compilado como módulo do crate
// `dartforge-runtime` (cfg `dartforge_runtime_embutido`, posta pelo build.rs),
// quem chama a entrada é o JIT, e um `main` C colidiria com o do binário Rust.
#[cfg(not(any(dartforge_runtime_embutido, dartforge_runtime_dll)))]
unsafe extern "C" {
    fn dartforge_entry();
}

/// Invoca uma vez o programa ligado ao runtime Rust.
#[cfg(not(any(dartforge_runtime_embutido, dartforge_runtime_dll)))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> i32 {
    // SAFETY: o objeto foi emitido para esta ABI e ligado pelo mesmo driver nativo.
    unsafe { dartforge_entry() };
    let codigo = finalizar_programa();
    if codigo != 0 {
        std::process::exit(codigo);
    }
    0
}

/// A entrada do programa com o SDK da fonte (P5c): o runtime e o SDK moram
/// numa DLL (cfg `dartforge_runtime_dll`, sem o `main` C), e o `main` do
/// executável — que o emissor escreve — chama esta função com a
/// `dartforge_entry` dele.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_iniciar(entrada: extern "C" fn(), para_texto: extern "C" fn(i64) -> i64) -> i32 {
    PARA_TEXTO.with(|p| p.set(Some(para_texto)));
    if depurar() {
        // Depuração: o pânico do runtime mostra a pilha de funções Dart
        // (`DARTFORGE_RASTRO=1` na compilação).
        let padrao = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |i| {
            mostrar_rastro();
            padrao(i);
        }));
    }
    // O isolado principal aceita os pedidos no ponto seguro (a recarga do
    // JIT) enquanto a entrada roda — o `main` e o laço de eventos.
    marcar_isolado_principal();
    entrada();
    desmarcar_isolado_principal();
    let codigo = finalizar_programa();
    if codigo != 0 {
        std::process::exit(codigo);
    }
    0
}

/// O que acontece depois que `dartforge_entry` retorna, nos DOIS perfis.
///
/// Exceção pendente: escreve `Uncaught exception: …` em stderr e devolve 101.
/// Senão, com `DARTFORGE_GC_STATS=1`, escreve as estatísticas do coletor, e
/// devolve o código de saída global (`exitCode` do `dart:io`; 0 sem ele). Quem chama encerra o processo com o código (o `main` acima, no
/// AOT; o executor, no JIT). Não chama `exit` aqui: é o único trecho do
/// runtime que os dois perfis dirigem, e fica escrito uma vez só.
pub fn finalizar_programa() -> i32 {
    // `Isolate.exit` no isolado principal: ele terminou, sem erro.
    let pending = EXCEPTION.with(|slot| slot.borrow().is_some()) && !desenrolando();
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
                _ => match PARA_TEXTO.with(|p| p.get()) {
                    // SDK da fonte: o `toString()` Dart do objeto lançado.
                    Some(f) => {
                        drop(heap);
                        EXCEPTION.with(|slot| slot.borrow_mut().take());
                        let t = com_raizes(&[bits], || f(bits));
                        let s = HEAP.with(|h| match h.borrow().try_get(t) {
                            Some(Value::String(texto)) => Some(texto.para_string()),
                            _ => None,
                        });
                        // O `toString()` também falhou: a descrição do
                        // runtime (a classe do objeto).
                        s.unwrap_or_else(|| {
                            EXCEPTION.with(|slot| slot.borrow_mut().take());
                            HEAP.with(|h| describe_handle(&h.borrow(), bits))
                        })
                    }
                    None => describe_handle(&heap, bits),
                },
            };
            use std::io::Write;
            let _ = writeln!(
                std::io::stderr().lock(),
                "Uncaught exception: {detail}"
            );
        });
        return 101;
    }
    if std::env::var("DARTFORGE_GC_STATS").as_deref() == Ok("1") {
        HEAP.with(|heap| {
            let s = heap.borrow().stats();
            eprintln!("{{\"dartforge_gc\":{{\"allocations\":{},\"collections\":{},\"reclaimed\":{},\"live_objects\":{},\"reserved_slots\":{},\"root_slots\":{},\"peak_root_slots\":{},\"live_roots\":{},\"peak_roots\":{},\"live_bytes\":{},\"peak_live_bytes\":{},\"permanent_roots\":{},\"smi_caixas_evitadas\":{}}}}}",
                s.allocations, s.collections, s.reclaimed, s.live_objects, s.reserved_slots,
                s.root_slots, s.peak_root_slots, s.live_roots, s.peak_roots,
                s.estimated_bytes, s.peak_estimated_bytes, s.permanent_roots, s.caixas_evitadas);
        });
    }
    codigo_de_saida_global()
}

use crate::heap::{Heap, TaggedValue, Texto, TextoMut, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

thread_local! {
    /// SDK da fonte: o `toString()` Dart de um valor (a
    /// `dartforge_dispatch_toString` do programa), para a exceção não
    /// capturada.
    static PARA_TEXTO: std::cell::Cell<Option<extern "C" fn(i64) -> i64>> = const { std::cell::Cell::new(None) };
    static HEAP: RefCell<Heap> = RefCell::new(Heap::new(std::env::var_os("DARTFORGE_GC_STRESS").is_some()));
    static CLASS_NAMES: RefCell<HashMap<i64, String>> = RefCell::new(HashMap::new());
    static SUBCLASSES: RefCell<HashMap<i64, Vec<i64>>> = RefCell::new(HashMap::new());
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

/// Registra relação de subtipagem direta: sub_id <: super_id.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_register_subclass(sub_id: i64, super_id: i64) {
    SUBCLASSES.with(|map| {
        // Sem duplicar: a publicação de uma recarga refaz os registros.
        let mut supers = map.borrow_mut();
        let lista = supers.entry(sub_id).or_default();
        if !lista.contains(&super_id) {
            lista.push(super_id);
        }
    });
}

/// Consulta pertinência de subtipagem nominal em tempo de execução.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_is_subclass(class_id: i64, target_class: i64) -> u8 {
    let r = is_subclass(class_id, target_class);
    if depurar() {
        eprintln!("[depurar] is_subclass({class_id}, {target_class}) = {r}");
    }
    r
}

fn is_subclass(class_id: i64, target_class: i64) -> u8 {
    if class_id == target_class {
        return 1;
    }
    if target_class == 0 {
        // Object é supertipo de toda classe nominal
        return 1;
    }
    SUBCLASSES.with(|map| {
        let map = map.borrow();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(class_id);
        visited.insert(class_id);
        while let Some(curr) = queue.pop_front() {
            if curr == target_class {
                return 1;
            }
            if let Some(supers) = map.get(&curr) {
                for &s in supers {
                    if visited.insert(s) {
                        queue.push_back(s);
                    }
                }
            }
        }
        0
    })
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
            tag: crate::heap::ValueTag::Double,
        },
        _ => panic!("tag de valor inválida"),
    }
}

/// Separa um valor na ABI plana (bits, tag) para chamadas LLVM.
fn untag(value: TaggedValue) -> (i64, u8) {
    use crate::heap::ValueTag;
    let tag = match value.tag {
        ValueTag::Int => 1,
        ValueTag::Bool => 2,
        ValueTag::Ref => 3,
        ValueTag::Double => 4,
    };
    (value.bits, tag)
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
            return 0;
        };
        let idx = usize::try_from(index).unwrap_or(usize::MAX);
        fields.get(idx).map_or(0, |(bits, _)| *bits)
    })
}
/// Grava campo e informa explicitamente se seus bits são referência gerenciada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_set(handle: i64, index: i64, bits: i64, is_ref: u8) {
    HEAP.with(|heap| heap.borrow_mut().set(handle, index, bits, is_ref != 0));
}

/// Estado de um campo `late` sem inicializador. O bit é independente dos
/// bits do campo: `0`, `false` e `null` podem ser valores já atribuídos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_initialized(handle: i64, index: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().campos_late_inicializados.contains(&(handle, index))))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_mark_initialized(handle: i64, index: i64) {
    HEAP.with(|heap| {
        heap.borrow_mut().campos_late_inicializados.insert((handle, index));
    });
}

/// O índice `-(index+2)` da mesma tabela lateral representa uma avaliação
/// em curso. `-1` fica reservado ao estado dos locais capturados em `Cell`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_initializing(handle: i64, index: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().campos_late_inicializados.contains(&(handle, -index - 2))))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_set_initializing(handle: i64, index: i64, active: u8) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let key = (handle, -index - 2);
        if active != 0 {
            heap.campos_late_inicializados.insert(key);
        } else {
            heap.campos_late_inicializados.remove(&key);
        }
    });
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

/// O valor escalar por trás de um `Ref` numérico ou booleano: `Smi`,
/// `_Mint`, `_Double` ou caixa de `bool` (R3/R10); outro valor dá `None`.
fn escalar_de_ref(heap: &Heap, r: i64) -> Option<TaggedValue> {
    if crate::heap::smi::e_smi(r) {
        return Some(TaggedValue::scalar(crate::heap::smi::valor(r)));
    }
    match heap.try_get(r) {
        Some(Value::BoxedInt(i)) => Some(TaggedValue::scalar(*i)),
        Some(Value::BoxedDouble(d)) => Some(TaggedValue::double(*d)),
        Some(Value::BoxedBool(b)) => Some(TaggedValue::boolean(*b)),
        _ => None,
    }
}

/// Compara igualdade (== de Dart) entre dois handles de referência.
///
/// Números comparam por valor, com a regra de `num`: `1 == 1.0` (R9); um
/// `Smi` e um `_Mint` nunca têm o mesmo valor (a forma é canônica, R10), mas
/// a comparação é por valor de todo modo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_equal(a: i64, b: i64) -> u8 {
    if a == b { return 1; }
    if a == 0 || b == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        use crate::heap::ValueTag::{Bool, Double, Int};
        match (escalar_de_ref(&heap, a), escalar_de_ref(&heap, b)) {
            (Some(x), Some(y)) => u8::from(match (x.tag, y.tag) {
                (Int, Int) | (Bool, Bool) => x.bits == y.bits,
                (Double, Double) => f64::from_bits(x.bits as u64) == f64::from_bits(y.bits as u64),
                (Int, Double) => (x.bits as f64) == f64::from_bits(y.bits as u64),
                (Double, Int) => f64::from_bits(x.bits as u64) == (y.bits as f64),
                _ => false,
            }),
            (Some(_), None) | (None, Some(_)) => 0,
            (None, None) => u8::from(heap.string_equal(a, b)),
        }
    })
}

/// `identical(a, b)` sobre referências, com a semântica da VM
/// (`Instance::IsIdenticalTo`): mesmo handle, ou dois inteiros de mesmo
/// valor, ou dois `double` bit a bit iguais (R9). Dois `Smi` de mesmo valor
/// têm os mesmos bits, e caem no primeiro teste.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_identical(a: i64, b: i64) -> u8 {
    if a == b { return 1; }
    if a == 0 || b == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        use crate::heap::ValueTag::{Double, Int};
        match (escalar_de_ref(&heap, a), escalar_de_ref(&heap, b)) {
            (Some(x), Some(y)) if (x.tag == Int && y.tag == Int) || (x.tag == Double && y.tag == Double) => {
                u8::from(x.bits == y.bits)
            }
            _ => 0,
        }
    })
}

/// `Box` (R3/R10): `int` numa posição `Ref` — o `Smi` quando cabe em 63
/// bits (não aloca), senão o `_Mint` no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_box_int(v: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().caixa_int(v))
}

/// `Box`: `double` numa posição `Ref`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_box_double(v: f64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::BoxedDouble(v)))
}

/// `Box`: `bool` numa posição `Ref` (um dos dois singletons).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_box_bool(v: u8) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().caixa_bool(v != 0))
}

/// Lança o `TypeError` de uma coerção implícita que falhou (`null` ou
/// outro tipo onde se esperava o escalar).
fn lancar_type_error() {
    let err = dartforge_type_error_new();
    dartforge_exception_throw(err, 3);
}

/// `Unbox` (R3/R10): `int` de uma referência (`Smi` ou `_Mint`); null ou
/// outro tipo lança TypeError.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_int(h: i64) -> i64 {
    if crate::heap::smi::e_smi(h) {
        return crate::heap::smi::valor(h);
    }
    let v = HEAP.with(|heap| heap.borrow().int_de_ref(h));
    v.unwrap_or_else(|| {
        lancar_type_error();
        0
    })
}

/// `Unbox`: `double` de uma referência (um `int` encaixotado não é `double`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_double(h: i64) -> f64 {
    let v = HEAP.with(|heap| match heap.borrow().try_get(h) {
        Some(Value::BoxedDouble(d)) => Some(*d),
        _ => None,
    });
    v.unwrap_or_else(|| {
        lancar_type_error();
        0.0
    })
}

/// `Unbox`: `bool` de uma referência.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_bool(h: i64) -> u8 {
    let v = HEAP.with(|heap| match heap.borrow().try_get(h) {
        Some(Value::BoxedBool(b)) => Some(*b),
        _ => None,
    });
    v.map_or_else(
        || {
            lancar_type_error();
            0
        },
        u8::from,
    )
}

/// Elemento como referência (R5/R8): escalar é encaixotado na saída.
fn valor_como_ref(v: TaggedValue) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().como_ref(v))
}

/// Consulta a classe nominal de um handle para testes `on T` de captura.
///
/// Devolve o `class_id` de objetos, -2 para strings, -3 para listas, -4 para
/// mapas, -5 para conjuntos e -6 para closures; outros valores internos nunca
/// são lançáveis pelo subconjunto e devolvem -1.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_value_class(handle: i64) -> i64 {
    // Com o SDK da fonte (P5c), os valores do runtime têm a classe do SDK
    // que representam (`_Smi`, `_OneByteString`, `_GrowableList`…).
    if let Some(cid) = cid_do_runtime(handle) {
        return cid;
    }
    // null tem classe própria (`Null`): os testes de tipo sobre `Ref`
    // perguntam a classe sem precisar desviar antes (R, testar_tipo).
    if handle == 0 {
        return -12;
    }
    // Um `Smi` é um `int` (a mesma classe do `_Mint`, R10).
    if crate::heap::smi::e_smi(handle) {
        return -9;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::Object { class_id, .. } => *class_id,
            Value::TypedData { class_id, .. } | Value::TypedView { class_id, .. } => *class_id,
            Value::String(_) => -2,
            Value::StringBuffer(_) => -8,
            Value::List(_) => -3,
            Value::Map(_) => -4,
            Value::Set(_) => -5,
            Value::Closure { .. } => -6,
            Value::BoxedInt(_) => -9,
            Value::BoxedDouble(_) => -10,
            Value::BoxedBool(_) => -11,
            Value::Record(_) => -7,
            Value::Cell(_) | Value::Environment(_) | Value::RegExp(_) | Value::Match(_) => -1,
        }
    })
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
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let items: Vec<TaggedValue> = items.into_iter().map(|v| heap.normalizar(v)).collect();
        heap.allocate(Value::Record(items))
    })
}
