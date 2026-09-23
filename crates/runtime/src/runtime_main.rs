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
use std::collections::{HashMap, HashSet};

thread_local! {
    static HEAP: RefCell<Heap> = RefCell::new(Heap::new(std::env::var_os("DARTFORGE_GC_STRESS").is_some()));
    static CLASS_NAMES: RefCell<HashMap<i64, String>> = RefCell::new(HashMap::new());
    static SUBCLASSES: RefCell<HashMap<i64, Vec<i64>>> = RefCell::new(HashMap::new());
}

// As tabelas laterais por handle (coleções imutáveis, iterações ativas,
// origem das listas de chaves) moram no `Heap`, que as purga a cada coleta
// (G6, docs/NATIVO-PLANO.md §6.5).

fn register_key_iteration_origin(keys_list: i64, map_handle: i64) {
    HEAP.with(|h| h.borrow_mut().origens.insert(keys_list, map_handle));
}

fn is_active_iteration(handle: i64) -> bool {
    HEAP.with(|h| h.borrow().iteracoes_ativas.contains(&handle))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_iteration_begin(handle: i64) {
    if handle == 0 { return; }
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.iteracoes_ativas.insert(handle);
        if let Some(&origin) = h.origens.get(&handle) {
            h.iteracoes_ativas.insert(origin);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_iteration_end(handle: i64) {
    if handle == 0 { return; }
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.iteracoes_ativas.remove(&handle);
        if let Some(&origin) = h.origens.get(&handle) {
            h.iteracoes_ativas.remove(&origin);
        }
    });
}

/// Executa `f` com `handles` enraizados num frame temporário (G6).
///
/// Extern que aloca mais de uma vez: o primeiro objeto só é referenciado
/// por uma variável do Rust enquanto o segundo é alocado, e essa alocação
/// pode coletar. O frame é o mesmo protocolo do código gerado.
fn com_raizes<R>(handles: &[i64], f: impl FnOnce() -> R) -> R {
    let frame = HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let frame = h.push_frame_with_slots(handles.len());
        for (i, &x) in handles.iter().enumerate() {
            h.set_root(frame, i, x);
        }
        frame
    });
    let r = f();
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    r
}

fn id_da_classe_stack_trace() -> i64 {
    CLASS_NAMES.with(|map| {
        map.borrow().iter().find(|(_, name)| *name == "StackTrace" || *name == "_StackTrace").map(|(&id, _)| id)
    }).unwrap_or(1006)
}

/// Objeto `StackTrace` com o texto dado (duas alocações, a primeira
/// enraizada durante a segunda).
fn alocar_stack_trace(texto: &str) -> i64 {
    let trace_str = HEAP.with(|h| h.borrow_mut().allocate(Value::String(texto.to_string())));
    let cid = id_da_classe_stack_trace();
    com_raizes(&[trace_str], || {
        HEAP.with(|h| h.borrow_mut().allocate(Value::Object { class_id: cid, fields: vec![(trace_str, true)] }))
    })
}

/// Objeto de erro com o rastro corrente no campo final (o `st` enraizado
/// enquanto o objeto é alocado).
fn alocar_erro_com_rastro(class_id: i64, mut campos: Vec<(i64, bool)>) -> i64 {
    let raizes: Vec<i64> = campos.iter().filter(|(_, r)| *r).map(|(b, _)| *b).collect();
    com_raizes(&raizes, || {
        let st = dartforge_stack_trace_get();
        campos.push((st, true));
        com_raizes(&[st], || HEAP.with(|h| h.borrow_mut().allocate(Value::Object { class_id, fields: campos })))
    })
}

/// Mensagem alocada, enraizada, e o erro com ela e o rastro.
fn alocar_erro_com_mensagem(class_id: i64, mensagem: &str, antes: Vec<(i64, bool)>, depois: Vec<(i64, bool)>) -> i64 {
    let msg = HEAP.with(|h| h.borrow_mut().allocate(Value::String(mensagem.to_string())));
    com_raizes(&[msg], || {
        let mut campos = antes;
        campos.push((msg, true));
        campos.extend(depois);
        alocar_erro_com_rastro(class_id, campos)
    })
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
        map.borrow_mut().entry(sub_id).or_default().push(super_id);
    });
}

/// Consulta pertinência de subtipagem nominal em tempo de execução.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_is_subclass(class_id: i64, target_class: i64) -> u8 {
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

thread_local! {
    static CURRENT_STACK_TRACE: RefCell<Option<i64>> = RefCell::new(None);
}

/// Retorna um objeto StackTrace não vazio gerenciado no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_stack_trace_get() -> i64 {
    if let Some(h) = CURRENT_STACK_TRACE.with(|slot| *slot.borrow()) {
        return h;
    }
    alocar_stack_trace("#0      main (dart:native)\n")
}

/// Retorna um objeto StackTrace vazio gerenciado no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_stack_trace_empty() -> i64 {
    alocar_stack_trace("")
}

/// Cria um objeto StackTrace a partir de uma string customizada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_stack_trace_from_string(str_handle: i64) -> i64 {
    HEAP.with(|heap| {
        let stack_trace_cid = CLASS_NAMES.with(|map| {
            map.borrow().iter().find(|(_, name)| *name == "StackTrace" || *name == "_StackTrace").map(|(&id, _)| id)
        }).unwrap_or(1006);
        heap.borrow_mut().allocate(Value::Object {
            class_id: stack_trace_cid,
            fields: vec![(str_handle, true)],
        })
    })
}

/// Lança exceção associando um stack trace explícito.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_throw_with_stack_trace(bits: i64, tag: u8, st_handle: i64) {
    CURRENT_STACK_TRACE.with(|slot| *slot.borrow_mut() = Some(st_handle));
    HEAP.with(|h| h.borrow_mut().set_raiz_do_runtime(1, st_handle));
    dartforge_exception_throw(bits, tag);
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
/// Valor corrente de um global `Ref` do programa, mantido como raiz permanente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_gc_global_root(id: i64, handle: i64) {
    HEAP.with(|heap| heap.borrow_mut().set_global_root(id, handle));
}
/// Programa que não compilou (construto não suportado): imprime os
/// diagnósticos que o lowering produziu e sai com 254.
///
/// # Safety
/// `ptr` aponta para `len` bytes UTF-8 de uma constante do módulo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_erro_de_compilacao(ptr: *const u8, len: i64) {
    let len = usize::try_from(len).expect("comprimento inválido");
    // SAFETY: constante LLVM legível pelo comprimento informado.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    use std::io::Write;
    let _ = std::io::stderr().lock().write_all(bytes);
    std::process::exit(254);
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
///
/// Caixas comparam por valor, com a regra de `num`: `1 == 1.0` (R9).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_equal(a: i64, b: i64) -> u8 {
    if a == b { return 1; }
    if a == 0 || b == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match (heap.get(a), heap.get(b)) {
            (Value::BoxedInt(x), Value::BoxedInt(y)) => u8::from(x == y),
            (Value::BoxedDouble(x), Value::BoxedDouble(y)) => u8::from(x == y),
            (Value::BoxedInt(x), Value::BoxedDouble(y)) | (Value::BoxedDouble(y), Value::BoxedInt(x)) => {
                u8::from((*x as f64) == *y)
            }
            (Value::BoxedBool(x), Value::BoxedBool(y)) => u8::from(x == y),
            _ => u8::from(heap.string_equal(a, b)),
        }
    })
}

/// `identical(a, b)` sobre referências, com a semântica da VM
/// (`Instance::IsIdenticalTo`): mesmo handle, ou dois inteiros de mesmo
/// valor, ou dois `double` bit a bit iguais (R9).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_identical(a: i64, b: i64) -> u8 {
    if a == b { return 1; }
    if a == 0 || b == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match (heap.get(a), heap.get(b)) {
            (Value::BoxedInt(x), Value::BoxedInt(y)) => u8::from(x == y),
            (Value::BoxedDouble(x), Value::BoxedDouble(y)) => u8::from(x.to_bits() == y.to_bits()),
            _ => 0,
        }
    })
}

/// `double` como a VM imprime (`1.0`, `0.5`, `NaN`, `Infinity`).
fn formatar_double(d: f64) -> String {
    if d.is_nan() {
        "NaN".to_string()
    } else if d.is_infinite() {
        if d > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }
    } else if d.fract() == 0.0 {
        format!("{d:.1}")
    } else {
        d.to_string()
    }
}

/// `Box` (R3): `int` numa posição `Ref`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_box_int(v: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::BoxedInt(v)))
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

/// `Unbox` (R3): `int` de uma referência; null ou outro tipo lança TypeError.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_int(h: i64) -> i64 {
    let v = HEAP.with(|heap| match heap.borrow().try_get(h) {
        Some(Value::BoxedInt(i)) => Some(*i),
        _ => None,
    });
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

/// `lista[i]` numa posição `Ref`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_get_ref(handle: i64, index: i64) -> i64 {
    let v = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::List(items) => usize::try_from(index).ok().and_then(|i| items.get(i).copied()),
            _ => None,
        }
    });
    match v {
        Some(v) => valor_como_ref(v),
        None => {
            // Fora dos limites: mesmo RangeError do acessor de bits.
            dartforge_list_get_bits(handle, index)
        }
    }
}

/// `first`/`last`/`single` numa posição `Ref`: o acessor de bits valida e
/// lança; aqui só se lê o elemento com a tag.
fn elemento_extremo_ref(handle: i64, qual: u8) -> i64 {
    let v = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::List(items) => match qual {
                0 => items.first().copied(),
                1 => items.last().copied(),
                _ => (items.len() == 1).then(|| items[0]),
            },
            _ => None,
        }
    });
    match (v, qual) {
        (Some(v), _) => valor_como_ref(v),
        (None, 0) => dartforge_list_first(handle),
        (None, 1) => dartforge_list_last(handle),
        (None, _) => dartforge_list_single(handle),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_first_ref(handle: i64) -> i64 {
    elemento_extremo_ref(handle, 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_last_ref(handle: i64) -> i64 {
    elemento_extremo_ref(handle, 1)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_single_ref(handle: i64) -> i64 {
    elemento_extremo_ref(handle, 2)
}

/// `mapa[chave]` (sempre anulável, logo `Ref`): ausente é null.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_get_ref(handle: i64, bits: i64, tag: u8) -> i64 {
    let key = tagged(bits, tag);
    let v = HEAP.with(|heap| {
        let heap = heap.borrow();
        heap.map_contains(handle, key).then(|| heap.map_get(handle, key))
    });
    v.map_or(0, valor_como_ref)
}

/// A exceção pendente como referência (o valor da variável do `catch`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_peek_ref() -> i64 {
    match EXCEPTION.with(|slot| *slot.borrow()) {
        Some(v) => valor_como_ref(v),
        None => 0,
    }
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

/// `Error.safeToString` do `dart:core`.
///
/// Referência: `sdk/lib/core/errors.dart`. Ela existe justamente para NÃO
/// chamar o `toString` do objeto — um `toString` que lança dentro da
/// construção de uma mensagem de erro esconderia o erro original. Portanto:
/// números, `bool` e `null` pelo `toString` deles; `String` entre aspas com
/// escapes; qualquer outra coisa pela forma de identidade do objeto.
///
/// As formas de identidade abaixo são as que a VM 3.6.2 imprime para as
/// coleções embutidas (`Instance(length:N) of '_GrowableList'`, `_Map len:N`,
/// `_Set len:N`), verificadas no oráculo; o corpus compara byte a byte.
fn safe_to_string(heap: &Heap, handle: i64, output: &mut String) {
    if handle == 0 {
        output.push_str("null");
        return;
    }
    match heap.get(handle) {
        Value::String(text) => {
            output.push('"');
            for c in text.chars() {
                match c {
                    '\n' => output.push_str("\\n"),
                    '\r' => output.push_str("\\r"),
                    '\t' => output.push_str("\\t"),
                    '\"' => output.push_str("\\\""),
                    '\\' => output.push_str("\\\\"),
                    c => output.push(c),
                }
            }
            output.push('"');
        }
        Value::List(items) => {
            output.push_str(&format!("Instance(length:{}) of '_GrowableList'", items.len()))
        }
        Value::Map(pares) => output.push_str(&format!("_Map len:{}", pares.len())),
        Value::BoxedInt(i) => output.push_str(&i.to_string()),
        Value::BoxedDouble(d) => output.push_str(&formatar_double(*d)),
        Value::BoxedBool(b) => output.push_str(if *b { "true" } else { "false" }),
        Value::Set(items) => output.push_str(&format!("_Set len:{}", items.len())),
        Value::Object { class_id, .. } => {
            let nome = CLASS_NAMES
                .with(|map| map.borrow().get(class_id).cloned())
                .unwrap_or_else(|| "Object".to_string());
            output.push_str(&format!("Instance of '{nome}'"));
        }
        _ => output.push_str(&describe_handle(heap, handle)),
    }
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
                Value::String(text) | Value::StringBuffer(text) | Value::RegExp(text) | Value::Match(text) => {
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
                // Caixas (R3): imprimem como o escalar que carregam.
                Value::BoxedInt(i) => output.push_str(&i.to_string()),
                Value::BoxedDouble(d) => output.push_str(&formatar_double(*d)),
                Value::BoxedBool(b) => output.push_str(if *b { "true" } else { "false" }),
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
                Value::Object { class_id, fields } => {
                    if *class_id == 1013 {
                        if let Some((b, _)) = fields.first() {
                            output.push_str(&format!("{b}"));
                        }
                        return;
                    }
                    let name = CLASS_NAMES.with(|map| map.borrow().get(class_id).cloned())
                        .unwrap_or_else(|| "Object".to_string());
                    if name == "Exception" || name == "_Exception" {
                        if !fields.is_empty() && fields[0].0 != 0 {
                            output.push_str("Exception: ");
                            let val = TaggedValue {
                                bits: fields[0].0,
                                is_ref: fields[0].1,
                                tag: if fields[0].1 { ValueTag::Ref } else { ValueTag::Int },
                            };
                            render(heap, val, depth - 1, output);
                        } else {
                            output.push_str("Exception");
                        }
                    } else if name == "FormatException" {
                        let m_opt = fields.first().and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let s_opt = fields.get(1).and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let off_opt = fields.get(2).and_then(|(b, _)| if *b >= 0 { Some(*b) } else { None });
                        if let Some(s) = s_opt {
                            let m = m_opt.unwrap_or_default();
                            if let Some(off) = off_opt {
                                let char_idx = off + 1;
                                let spaces = " ".repeat(off as usize);
                                if m.is_empty() {
                                    output.push_str(&format!("FormatException (at character {char_idx})\n{s}\n{spaces}^\n"));
                                } else {
                                    output.push_str(&format!("FormatException: {m} (at character {char_idx})\n{s}\n{spaces}^\n"));
                                }
                            } else if m.is_empty() {
                                output.push_str(&format!("FormatException\n{s}"));
                            } else {
                                output.push_str(&format!("FormatException: {m}\n{s}"));
                            }
                        } else if let Some(m) = m_opt {
                            if m.is_empty() {
                                output.push_str("FormatException");
                            } else {
                                output.push_str(&format!("FormatException: {m}"));
                            }
                        } else {
                            output.push_str("FormatException");
                        }
                    } else if name == "StateError" {
                        if let Some((b, _)) = fields.first() {
                            if *b != 0 {
                                let m = match heap.get(*b) { Value::String(s) => s.clone(), _ => "".to_string() };
                                output.push_str(&format!("Bad state: {m}"));
                            } else {
                                output.push_str("Bad state");
                            }
                        } else {
                            output.push_str("Bad state");
                        }
                    } else if name == "ArgumentError" {
                        let m_opt = fields.first().and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let n_opt = fields.get(1).and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let has_val = fields.get(3).map_or(false, |(b, _)| *b != 0);
                        if has_val {
                            let val_str = if let Some(&(v_bits, is_ref)) = fields.get(2) {
                                if is_ref {
                                    if v_bits == 0 {
                                        "null".to_string()
                                    } else {
                                        match heap.get(v_bits) {
                                            Value::String(s) => format!("\"{s}\""),
                                            _ => {
                                                let mut tmp = String::new();
                                                render(heap, TaggedValue::reference(v_bits), depth - 1, &mut tmp);
                                                tmp
                                            }
                                        }
                                    }
                                } else {
                                    v_bits.to_string()
                                }
                            } else {
                                "null".to_string()
                            };
                            match (n_opt, m_opt) {
                                (Some(n), Some(m)) => output.push_str(&format!("Invalid argument ({n}): {m}: {val_str}")),
                                (Some(n), None) => output.push_str(&format!("Invalid argument ({n}): {val_str}")),
                                (None, Some(m)) => output.push_str(&format!("Invalid argument: {m}: {val_str}")),
                                (None, None) => output.push_str(&format!("Invalid argument: {val_str}")),
                            }
                        } else {
                            match (n_opt, m_opt) {
                                (Some(n), Some(m)) => output.push_str(&format!("Invalid argument(s) ({n}): {m}")),
                                (Some(n), None) => output.push_str(&format!("Invalid argument(s) ({n})")),
                                (None, Some(m)) => output.push_str(&format!("Invalid argument(s): {m}")),
                                (None, None) => output.push_str("Invalid argument(s)"),
                            }
                        }
                    } else if name == "RangeError" {
                        let m_opt = fields.first().and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let n_opt = fields.get(1).and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let inv_val = fields.get(2).map_or(0, |(b, _)| *b);
                        let start_val = fields.get(3).map_or(0, |(b, _)| *b);
                        let end_val = fields.get(4).map_or(0, |(b, _)| *b);
                        let kind = fields.get(5).map_or(0, |(b, _)| *b);
                        let has_val = fields.get(6).map_or(false, |(b, _)| *b != 0);

                        if kind == 1 {
                            match (n_opt, m_opt) {
                                (Some(n), Some(m)) => output.push_str(&format!("RangeError ({n}): {m}: Not in inclusive range {start_val}..{end_val}: {inv_val}")),
                                (Some(n), None) => output.push_str(&format!("RangeError ({n}): Invalid value: Not in inclusive range {start_val}..{end_val}: {inv_val}")),
                                (None, Some(m)) => output.push_str(&format!("RangeError: {m}: Not in inclusive range {start_val}..{end_val}: {inv_val}")),
                                (None, None) => output.push_str(&format!("RangeError: Invalid value: Not in inclusive range {start_val}..{end_val}: {inv_val}")),
                            }
                        } else if kind == 2 {
                            match (n_opt, m_opt) {
                                (Some(n), Some(m)) => output.push_str(&format!("RangeError ({n}): {m}: index should be less than {end_val}: {inv_val}")),
                                (Some(n), None) => output.push_str(&format!("RangeError ({n}): Index out of range: index should be less than {end_val}: {inv_val}")),
                                (None, Some(m)) => output.push_str(&format!("RangeError: {m}: index should be less than {end_val}: {inv_val}")),
                                (None, None) => output.push_str(&format!("RangeError: Index out of range: index should be less than {end_val}: {inv_val}")),
                            }
                        } else if has_val {
                            match (n_opt, m_opt) {
                                (Some(n), Some(m)) => output.push_str(&format!("RangeError ({n}): {m}: {inv_val}")),
                                (Some(n), None) => output.push_str(&format!("RangeError ({n}): Value not in range: {inv_val}")),
                                (None, Some(m)) => output.push_str(&format!("RangeError: {m}: {inv_val}")),
                                (None, None) => output.push_str(&format!("RangeError: Value not in range: {inv_val}")),
                            }
                        } else if let Some(m) = m_opt {
                            output.push_str(&format!("RangeError: {m}"));
                        } else {
                            output.push_str("RangeError");
                        }
                    } else if name == "UnsupportedError" {
                        if let Some((b, _)) = fields.first() {
                            if *b != 0 {
                                let m = match heap.get(*b) { Value::String(s) => s.clone(), _ => "".to_string() };
                                output.push_str(&format!("Unsupported operation: {m}"));
                            } else {
                                output.push_str("Unsupported operation");
                            }
                        } else {
                            output.push_str("Unsupported operation");
                        }
                    } else if name == "UnimplementedError" {
                        if let Some((b, _)) = fields.first() {
                            if *b != 0 {
                                let m = match heap.get(*b) { Value::String(s) => s.clone(), _ => "".to_string() };
                                if m.is_empty() {
                                    output.push_str("UnimplementedError");
                                } else {
                                    output.push_str(&format!("UnimplementedError: {m}"));
                                }
                            } else {
                                output.push_str("UnimplementedError");
                            }
                        } else {
                            output.push_str("UnimplementedError");
                        }
                    } else if name == "AssertionError" {
                        if let Some(&(b, is_ref)) = fields.first() {
                            if b != 0 {
                                if is_ref {
                                    match heap.get(b) {
                                        Value::String(s) => output.push_str(&format!("Assertion failed: \"{s}\"")),
                                        _ => {
                                            let mut s = String::new();
                                            render(heap, TaggedValue::reference(b), depth - 1, &mut s);
                                            output.push_str(&format!("Assertion failed: {s}"));
                                        }
                                    }
                                } else {
                                    output.push_str(&format!("Assertion failed: {b}"));
                                }
                            } else {
                                output.push_str("Assertion failed");
                            }
                        } else {
                            output.push_str("Assertion failed");
                        }
                    } else if name == "ConcurrentModificationError" {
                        // dart:core/errors.dart: sem `modifiedObject` o texto
                        // termina no ponto; com ele entra
                        // `Error.safeToString(modifiedObject)`, que NÃO chama o
                        // `toString` do objeto — usa a forma de identidade da
                        // VM. Por isso a VM imprime
                        // `Instance(length:4) of '_GrowableList'` e não `[1,2,3,1]`.
                        let alvo = fields.first().map_or(0, |(b, _)| *b);
                        if alvo == 0 {
                            output.push_str("Concurrent modification during iteration.");
                        } else {
                            let mut s = String::new();
                            safe_to_string(heap, alvo, &mut s);
                            output.push_str(&format!("Concurrent modification during iteration: {s}."));
                        }
                    } else if name == "TypeError" {
                        output.push_str("TypeError");
                    } else if name == "NoSuchMethodError" {
                        // PENDENTE: a VM imprime
                        // "NoSuchMethodError: Class 'X' has no instance getter
                        // 'y'." — para isso falta o nome da classe do receptor
                        // no ponto do lancamento. Ate la, o nome do membro ja
                        // e o que torna a falha diagnosticavel no placar.
                        let nome_membro = fields
                            .first()
                            .and_then(|(b, _)| if *b != 0 {
                                match heap.get(*b) {
                                    Value::String(s) => Some(s.clone()),
                                    _ => None,
                                }
                            } else {
                                None
                            });
                        match nome_membro {
                            Some(m) => output.push_str(&format!("NoSuchMethodError: {m}")),
                            None => output.push_str("NoSuchMethodError"),
                        }
                    } else if name == "StackTrace" || name == "_StackTrace" {
                        if let Some(m) = fields.first().and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None }) {
                            output.push_str(&m);
                        } else {
                            output.push_str("#0      main (dart:native)\n");
                        }
                    } else {
                        output.push_str(&format!("Instance of '{name}'"));
                    }
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

fn allocate_state_error(message: &str) -> i64 {
    alocar_erro_com_mensagem(1002, message, Vec::new(), Vec::new()) // StateError
}

fn allocate_range_error(message: &str) -> i64 {
    alocar_erro_com_mensagem(1004, message, Vec::new(), Vec::new()) // RangeError
}

/// Lê o primeiro elemento da lista ou lança StateError se vazia.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_first(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if handle == 0 { return 0; }
        if let Value::List(items) = heap_ref.get(handle) {
            if items.is_empty() {
                drop(heap_ref);
                let err = allocate_state_error("No element");
                dartforge_exception_throw(err, 3);
                return 0;
            }
            items[0].bits
        } else {
            0
        }
    })
}

/// Lê o último elemento da lista ou lança StateError se vazia.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_last(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if handle == 0 { return 0; }
        if let Value::List(items) = heap_ref.get(handle) {
            if items.is_empty() {
                drop(heap_ref);
                let err = allocate_state_error("No element");
                dartforge_exception_throw(err, 3);
                return 0;
            }
            items.last().unwrap().bits
        } else {
            0
        }
    })
}

/// Lê os bits do elemento, sem verificar limites: o emissor verifica antes.
///
/// O índice fora dos limites lança `RangeError` capturável em vez de abortar;
/// o emissor desvia para o tratador ao observar a exceção pendente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_get_bits(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if handle == 0 { return 0; }
        match heap_ref.get(handle) {
            Value::List(_) => {
                if index < 0 || index >= heap_ref.list_len(handle) as i64 {
                    drop(heap_ref);
                    let err = allocate_range_error("RangeError");
                    dartforge_exception_throw(err, 3);
                    return 0;
                }
                heap_ref.list_get(handle, index as usize).bits
            }
            Value::String(s) => {
                let units: Vec<u16> = s.encode_utf16().collect();
                if index < 0 || index >= units.len() as i64 {
                    drop(heap_ref);
                    let err = allocate_range_error("RangeError");
                    dartforge_exception_throw(err, 3);
                    return 0;
                }
                let u = units[index as usize];
                drop(heap_ref);
                match String::from_utf16(&[u]) {
                    Ok(ch_str) => HEAP.with(|h| h.borrow_mut().allocate(Value::String(ch_str))),
                    Err(_) => HEAP.with(|h| h.borrow_mut().allocate(Value::RawString(vec![u]))),
                }
            }
            Value::RawString(r) => {
                if index < 0 || index >= r.len() as i64 {
                    drop(heap_ref);
                    let err = allocate_range_error("RangeError");
                    dartforge_exception_throw(err, 3);
                    return 0;
                }
                let u = r[index as usize];
                drop(heap_ref);
                HEAP.with(|h| h.borrow_mut().allocate(Value::RawString(vec![u])))
            }
            Value::Match(s) => {
                if index == 0 {
                    let s_clone = s.clone();
                    drop(heap_ref);
                    HEAP.with(|h| h.borrow_mut().allocate(Value::String(s_clone)))
                } else {
                    0
                }
            }
            _ => 0,
        }
    })
}

/// Lê a tag (1 = int, 2 = bool, 3 = referência) do elemento da lista ou string.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_get_tag(handle: i64, index: i64) -> u8 {
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if handle == 0 { return 0; }
        match heap_ref.get(handle) {
            Value::List(_) => {
                if index < 0 || index >= heap_ref.list_len(handle) as i64 {
                    return 0;
                }
                untag(heap_ref.list_get(handle, index as usize)).1
            }
            Value::String(_) | Value::RawString(_) | Value::Match(_) => 3,
            _ => 0,
        }
    })
}

/// Substitui o elemento existente; limites verificados como na leitura.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_set(handle: i64, index: i64, bits: i64, tag: u8) {
    if dartforge_collection_is_unmodifiable(handle) != 0 {
        let err = dartforge_unsupported_error_new(0);
        dartforge_exception_throw(err, 3);
        return;
    }
    let value = tagged(bits, tag);
    HEAP.with(|heap| {
        if index < 0 || index >= heap.borrow().list_len(handle) as i64 {
            let err = allocate_range_error("RangeError");
            dartforge_exception_throw(err, 3);
            return;
        }
        heap.borrow_mut().list_set(handle, index as usize, value);
    });
}

/// Acrescenta ao fim (`List.add`); devolve void pela ABI do emissor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_push(handle: i64, bits: i64, tag: u8) {
    if dartforge_collection_is_unmodifiable(handle) != 0 {
        let err = dartforge_unsupported_error_new(0);
        dartforge_exception_throw(err, 3);
        return;
    }
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
    if dartforge_collection_is_unmodifiable(handle) != 0 {
        let err = dartforge_unsupported_error_new(0);
        dartforge_exception_throw(err, 3);
        return;
    }
    if is_active_iteration(handle) {
        let err = dartforge_concurrent_modification_error_new(handle);
        dartforge_exception_throw(err, 3);
        return;
    }
    let key = tagged(key_bits, key_tag);
    let value = tagged(value_bits, value_tag);
    HEAP.with(|heap| heap.borrow_mut().map_set(handle, key, value));
}

/// Remove par do mapa; lança ConcurrentModificationError se em iteração ativa.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_remove(handle: i64, key_bits: i64, key_tag: u8) -> i64 {
    if handle == 0 { return 0; }
    if is_active_iteration(handle) {
        let err = dartforge_concurrent_modification_error_new(handle);
        dartforge_exception_throw(err, 3);
        return 0;
    }
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let target_key = TaggedValue {
            bits: key_bits,
            is_ref: key_tag == 3,
            tag: match key_tag {
                1 => heap::ValueTag::Int,
                2 => heap::ValueTag::Bool,
                3 => heap::ValueTag::Ref,
                _ => heap::ValueTag::Int,
            },
        };
        let Value::Map(entries) = heap.get_mut(handle) else { return 0; };
        if let Some(pos) = entries.iter().position(|(k, _)| k.bits == target_key.bits) {
            entries.remove(pos).1.bits
        } else {
            0
        }
    })
}

/// Retorna lista com as chaves do mapa, associando a origem para rastreamento de iteração.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_keys(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        let Value::Map(entries) = heap_ref.get(handle) else { return 0; };
        let keys: Vec<TaggedValue> = entries.iter().map(|(k, _)| *k).collect();
        drop(heap_ref);
        let list_handle = heap.borrow_mut().create_list(keys);
        register_key_iteration_origin(list_handle, handle);
        list_handle
    })
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
        // SAFETY: vetores temporários do emissor, legíveis pelos `2 * len` i64.
        let raw = unsafe { std::slice::from_raw_parts(pairs, len * 2) };
        raw.chunks_exact(2)
            .map(|pair| tagged(pair[0], u8::try_from(pair[1]).expect("tag inválida")))
            .collect()
    };
    HEAP.with(|heap| heap.borrow_mut().create_set(values))
}

/// Quantidade de elementos únicos do conjunto.
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
    if is_active_iteration(handle) {
        let err = dartforge_concurrent_modification_error_new(handle);
        dartforge_exception_throw(err, 3);
        return 0;
    }
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
    // null tem classe própria (`Null`): os testes de tipo sobre `Ref`
    // perguntam a classe sem precisar desviar antes (R, testar_tipo).
    if handle == 0 {
        return -12;
    }
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
            Value::BoxedInt(_) => -9,
            Value::BoxedDouble(_) => -10,
            Value::BoxedBool(_) => -11,
            Value::Record(_) => -7,
            Value::Cell(_) | Value::Environment(_) | Value::RegExp(_) | Value::Match(_) => -1,
        }
    })
}

/// Registra a exceção pendente; referências devem estar vivas e enraizadas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_throw(bits: i64, tag: u8) {
    let value = tagged(bits, tag);
    if value.is_ref && value.bits != 0 {
        // Só os erros que o próprio runtime representa (ids 1000–1012) têm o
        // campo do rastro; um objeto do usuário que estende `Error` tem o
        // layout dele (R7) e não pode ganhar campo aqui. O rastro é alocado
        // SEM empréstimo do heap aberto (G6: a versão anterior chamava
        // `dartforge_stack_trace_get` com `borrow_mut` ativo — "RefCell
        // already borrowed").
        let precisa = HEAP.with(|heap| match heap.borrow().get(value.bits) {
            Value::Object { class_id, fields } if (1000..=1012).contains(class_id) && dartforge_is_subclass(*class_id, 1007) != 0 => {
                let st_idx = match *class_id {
                    1003 => 5,
                    1004 => 7,
                    _ => 1,
                };
                (fields.get(st_idx).map_or(0, |f| f.0) == 0).then_some(st_idx)
            }
            _ => None,
        });
        if let Some(st_idx) = precisa {
            let st = dartforge_stack_trace_get();
            HEAP.with(|heap| {
                if let Value::Object { fields, .. } = heap.borrow_mut().get_mut(value.bits) {
                    if fields.len() <= st_idx {
                        fields.resize(st_idx + 1, (0, false));
                    }
                    fields[st_idx] = (st, true);
                }
            });
        }
    }
    // G6: a exceção pendente é raiz até ser consumida.
    HEAP.with(|h| h.borrow_mut().set_raiz_do_runtime(0, if value.is_ref { value.bits } else { 0 }));
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
    HEAP.with(|h| h.borrow_mut().set_raiz_do_runtime(0, 0));
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

/// Inspeciona os bits da exceção pendente sem consumir/limpar o slot.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_peek_bits() -> i64 {
    EXCEPTION.with(|slot| slot.borrow().map_or(0, |value| value.bits))
}

/// Inspeciona a tag da exceção pendente sem consumir/limpar o slot.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_peek_tag() -> u8 {
    EXCEPTION.with(|slot| slot.borrow().map_or(0, |value| untag(value).1))
}

/// Desarma e limpa o slot de exceção pendente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_clear() {
    EXCEPTION.with(|slot| {
        slot.borrow_mut().take();
    });
    CURRENT_STACK_TRACE.with(|slot| {
        slot.borrow_mut().take();
    });
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.set_raiz_do_runtime(0, 0);
        h.set_raiz_do_runtime(1, 0);
    });
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
    let len = units.len() as i64;
    let actual_end = if end < 0 { len } else { end };
    if start < 0 || start > len || actual_end < start || actual_end > len {
        let err = dartforge_range_error_value(start, 0, 0);
        dartforge_exception_throw(err, 3);
        return 0;
    }
    let start_idx = start as usize;
    let end_idx = actual_end as usize;
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
        let start_pos = (start.max(0) as usize).min(target_units.len());
        if pat_units.is_empty() { return start_pos as i64; }
        for i in start_pos..=target_units.len().saturating_sub(pat_units.len()) {
            if target_units[i..i + pat_units.len()] == pat_units[..] {
                return i as i64;
            }
        }
        -1
    })
}

/// Localiza última ocorrência de substring a partir de `start`. Devolve -1 se não encontrar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_last_index_of(handle: i64, pat_handle: i64, start: i64) -> i64 {
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
        let max_pos = if start < 0 {
            target_units.len()
        } else {
            (start as usize).min(target_units.len())
        };
        if pat_units.is_empty() { return max_pos as i64; }
        if target_units.len() < pat_units.len() { return -1; }
        let search_start = max_pos.min(target_units.len() - pat_units.len());
        for i in (0..=search_start).rev() {
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
    let res_items: Vec<Value> = HEAP.with(|heap| {
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
        pieces
    });
    lista_de_pedacos(res_items.into_iter().map(|v| (None, v)).collect())
}

/// Lista com os valores dados, na ordem, alocando cada um já com a lista
/// enraizada (G6): antes os pedaços iam para um `Vec` do Rust e a lista era
/// alocada no fim — uma coleta no meio liberava os primeiros.
fn lista_de_pedacos(itens: Vec<(Option<TaggedValue>, Value)>) -> i64 {
    let lista = HEAP.with(|h| h.borrow_mut().create_list(Vec::new()));
    com_raizes(&[lista], || {
        for (antes, val) in itens {
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                if let Some(a) = antes {
                    h.list_push(lista, a);
                }
                let x = h.allocate(val);
                h.list_push(lista, TaggedValue::reference(x));
            });
        }
    });
    lista
}

/// Verifica se a string contém a substring a partir de `start`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_contains(handle: i64, pat_handle: i64, start: i64) -> u8 {
    (dartforge_string_index_of(handle, pat_handle, start) >= 0) as u8
}

fn match_pattern_at(text: &str, pat: &str) -> Option<usize> {
    if pat == r"\d+" || pat == "\\d+" {
        let mut len = 0;
        for c in text.chars() {
            if c.is_ascii_digit() {
                len += c.len_utf8();
            } else {
                break;
            }
        }
        if len > 0 { Some(len) } else { None }
    } else if pat.starts_with('[') && pat.ends_with(']') && pat.len() >= 2 {
        let set = &pat[1..pat.len() - 1];
        if let Some(first_char) = text.chars().next() {
            if set.contains(first_char) {
                Some(first_char.len_utf8())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        if text.starts_with(pat) {
            Some(pat.len())
        } else {
            None
        }
    }
}

fn find_next_pattern(text: &str, start_byte: usize, pat: &str) -> Option<(usize, usize)> {
    let mut byte_idx = start_byte;
    while byte_idx < text.len() {
        if let Some(len) = match_pattern_at(&text[byte_idx..], pat) {
            return Some((byte_idx, len));
        }
        let ch = text[byte_idx..].chars().next().unwrap();
        byte_idx += ch.len_utf8();
    }
    None
}

/// Cria um novo RegExp.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_regexp_new(pat_handle: i64) -> i64 {
    let pat = HEAP.with(|heap| {
        let heap = heap.borrow();
        if pat_handle != 0 {
            if let Value::String(s) = heap.get(pat_handle) { s.clone() } else { String::new() }
        } else {
            String::new()
        }
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::RegExp(pat)))
}

/// Divide a string em pedaços para splitMapJoin: retorna lista de [is_match (bool), part (Match ou String)].
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_split_map_pieces(target_handle: i64, pat_handle: i64) -> i64 {
    if target_handle == 0 {
        return HEAP.with(|h| h.borrow_mut().allocate(Value::List(Vec::new())));
    }
    let (target_str, pat_str) = HEAP.with(|heap| {
        let heap = heap.borrow();
        let target = match heap.get(target_handle) {
            Value::String(s) => s.clone(),
            _ => String::new(),
        };
        let pat = match heap.get(pat_handle) {
            Value::String(s) => s.clone(),
            Value::RegExp(p) => p.clone(),
            _ => String::new(),
        };
        (target, pat)
    });

    let mut pieces = Vec::new();
    let mut curr_byte = 0;
    while curr_byte <= target_str.len() {
        if let Some((match_start, match_len)) = find_next_pattern(&target_str, curr_byte, &pat_str) {
            let non_match = &target_str[curr_byte..match_start];
            pieces.push((false, non_match.to_string(), false));

            let matched = &target_str[match_start..match_start + match_len];
            pieces.push((true, matched.to_string(), true));

            if match_len > 0 {
                curr_byte = match_start + match_len;
            } else if let Some(ch) = target_str[match_start..].chars().next() {
                curr_byte = match_start + ch.len_utf8();
            } else {
                break;
            }
        } else {
            let non_match = &target_str[curr_byte..];
            pieces.push((false, non_match.to_string(), false));
            break;
        }
    }

    lista_de_pedacos(
        pieces
            .into_iter()
            .map(|(is_match, text, is_match_obj)| {
                let val = if is_match_obj { Value::Match(text) } else { Value::String(text) };
                (Some(TaggedValue::boolean(is_match)), val)
            })
            .collect(),
    )
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
        let to = if to_handle != 0 {
            if let Value::String(t) = heap.get(to_handle) { t.as_str() } else { "" }
        } else { "" };

        if from_handle != 0 {
            match heap.get(from_handle) {
                Value::String(from) => s.replace(from, to),
                Value::RegExp(pat) => {
                    let mut out = String::new();
                    let mut curr = 0;
                    while curr < s.len() {
                        if let Some((start, len)) = find_next_pattern(s, curr, pat) {
                            out.push_str(&s[curr..start]);
                            out.push_str(to);
                            if len > 0 {
                                curr = start + len;
                            } else if let Some(ch) = s[start..].chars().next() {
                                out.push(ch);
                                curr = start + ch.len_utf8();
                            } else {
                                break;
                            }
                        } else {
                            out.push_str(&s[curr..]);
                            break;
                        }
                    }
                    out
                }
                _ => s.to_string(),
            }
        } else {
            s.to_string()
        }
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(res_str)))
}

/// Preenche à esquerda até a largura indicada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_pad_left(handle: i64, width: i64, pad_handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let pad_str = HEAP.with(|heap| {
        let heap = heap.borrow();
        if pad_handle != 0 {
            if let Value::String(p) = heap.get(pad_handle) { p.clone() } else { " ".to_string() }
        } else { " ".to_string() }
    });
    let s_len = dartforge_generic_len(handle);
    if width <= s_len { return handle; }
    let delta = (width - s_len) as usize;
    if pad_str.is_empty() { return handle; }
    let pad_repeated = pad_str.repeat(delta);
    let pad_h = HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(pad_repeated)));
    HEAP.with(|heap| heap.borrow_mut().string_concat(pad_h, handle))
}

/// Preenche à direita até a largura indicada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_pad_right(handle: i64, width: i64, pad_handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let pad_str = HEAP.with(|heap| {
        let heap = heap.borrow();
        if pad_handle != 0 {
            if let Value::String(p) = heap.get(pad_handle) { p.clone() } else { " ".to_string() }
        } else { " ".to_string() }
    });
    let s_len = dartforge_generic_len(handle);
    if width <= s_len { return handle; }
    let delta = (width - s_len) as usize;
    if pad_str.is_empty() { return handle; }
    let pad_repeated = pad_str.repeat(delta);
    let pad_h = HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(pad_repeated)));
    HEAP.with(|heap| heap.borrow_mut().string_concat(handle, pad_h))
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

fn is_dart_whitespace(c: char) -> bool {
    matches!(c,
        '\t' | '\n' | '\x0B' | '\x0C' | '\r' | ' ' |
        '\u{0085}' | '\u{00A0}' | '\u{1680}' |
        '\u{2000}'..='\u{200A}' |
        '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
    )
}

/// Remove espaços em branco do início e fim.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_trim(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let trimmed = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.trim_matches(is_dart_whitespace).to_string()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(trimmed)))
}

/// Remove espaços em branco do início.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_trim_left(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let trimmed = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.trim_start_matches(is_dart_whitespace).to_string()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(trimmed)))
}

/// Remove espaços em branco do fim.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_trim_right(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let trimmed = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.trim_end_matches(is_dart_whitespace).to_string()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(trimmed)))
}

/// Verifica se a string começa com o prefixo dado a partir do índice `start`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_starts_with(handle: i64, pat_handle: i64, start: i64) -> u8 {
    if handle == 0 || pat_handle == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            Value::StringBuffer(b) => b.encode_utf16().collect(),
            _ => return 0,
        };
        let pat_units: Vec<u16> = match heap.get(pat_handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return 0,
        };
        if start < 0 { return 0; }
        let start_pos = start as usize;
        if start_pos > target_units.len() { return 0; }
        if start_pos + pat_units.len() > target_units.len() { return 0; }
        (target_units[start_pos..start_pos + pat_units.len()] == pat_units[..]) as u8
    })
}

/// Verifica se a string termina com o sufixo dado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_ends_with(handle: i64, pat_handle: i64) -> u8 {
    if handle == 0 || pat_handle == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            Value::StringBuffer(b) => b.encode_utf16().collect(),
            _ => return 0,
        };
        let pat_units: Vec<u16> = match heap.get(pat_handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return 0,
        };
        if pat_units.len() > target_units.len() { return 0; }
        target_units.ends_with(&pat_units) as u8
    })
}

/// Retorna uma nova string convertida para minúsculas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_to_lower(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.to_lowercase()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(text)))
}

/// Compara duas strings lexicograficamente segundo unidades UTF-16 (-1, 0, 1).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_compare_to(handle: i64, other_handle: i64) -> i64 {
    if handle == 0 && other_handle == 0 { return 0; }
    if handle == 0 { return -1; }
    if other_handle == 0 { return 1; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return 0,
        };
        let other_units: Vec<u16> = match heap.get(other_handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return 0,
        };
        match target_units.cmp(&other_units) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    })
}

/// Substitui a primeira ocorrência de `from` por `to` a partir de `start`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_replace_first(handle: i64, from_handle: i64, to_handle: i64, start: i64) -> i64 {
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
        if from.is_empty() {
            let start_idx = (start.max(0) as usize).min(s.len());
            return format!("{}{}{}", &s[..start_idx], to, &s[start_idx..]);
        }
        let search_start = (start.max(0) as usize).min(s.len());
        if let Some(pos) = s[search_start..].find(from) {
            let actual_pos = search_start + pos;
            format!("{}{}{}", &s[..actual_pos], to, &s[actual_pos + from.len()..])
        } else {
            s.to_string()
        }
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(res_str)))
}

/// Substitui a faixa [start, end) por `replacement`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_replace_range(handle: i64, start: i64, end: i64, rep_handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let res = HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return String::new(),
        };
        let rep_units: Vec<u16> = if rep_handle != 0 {
            match heap.get(rep_handle) {
                Value::String(s) => s.encode_utf16().collect(),
                Value::RawString(r) => r.clone(),
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let start_pos = (start.max(0) as usize).min(target_units.len());
        let end_pos = (end.max(0) as usize).min(target_units.len());
        let mut out = Vec::new();
        out.extend_from_slice(&target_units[..start_pos]);
        out.extend_from_slice(&rep_units);
        if end_pos < target_units.len() {
            out.extend_from_slice(&target_units[end_pos..]);
        }
        String::from_utf16_lossy(&out)
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(res)))
}

fn allocate_format_exception(message: &str) -> i64 {
    let msg = HEAP.with(|h| h.borrow_mut().allocate(Value::String(message.to_string())));
    com_raizes(&[msg], || {
        HEAP.with(|h| {
            h.borrow_mut().allocate(Value::Object {
                class_id: 1001, // FormatException
                fields: vec![(msg, true), (0, true), (-1, false)],
            })
        })
    })
}

/// Converte string para inteiro ou lança FormatException.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_int_parse(handle: i64) -> i64 {
    if handle == 0 {
        let err = allocate_format_exception("Invalid number: null");
        dartforge_exception_throw(err, 3);
        return 0;
    }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.trim().to_string(),
            _ => String::new(),
        }
    });
    match text.parse::<i64>() {
        Ok(val) => val,
        Err(_) => {
            let err = allocate_format_exception(&format!("Invalid radix-10 number: {text}"));
            dartforge_exception_throw(err, 3);
            0
        }
    }
}

/// Tenta converter string para inteiro; se falhar, devolve 0 (null); se passar, aloca _BoxedInt (1013).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_int_try_parse(handle: i64) -> i64 {
    if handle == 0 {
        return 0;
    }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.trim().to_string(),
            _ => String::new(),
        }
    });
    match text.parse::<i64>() {
        Ok(val) => HEAP.with(|heap| heap.borrow_mut().allocate(Value::BoxedInt(val))),
        Err(_) => 0,
    }
}

/// Converte string para ponto flutuante ou lança FormatException.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_double_parse(handle: i64) -> f64 {
    if handle == 0 {
        let err = allocate_format_exception("Invalid double: null");
        dartforge_exception_throw(err, 3);
        return 0.0;
    }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.trim().to_string(),
            _ => String::new(),
        }
    });
    match text.parse::<f64>() {
        Ok(val) => val,
        Err(_) => {
            let err = allocate_format_exception(&format!("Invalid double: {text}"));
            dartforge_exception_throw(err, 3);
            0.0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_collection_mark_unmodifiable(handle: i64) -> i64 {
    HEAP.with(|h| h.borrow_mut().imutaveis.insert(handle));
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_collection_is_unmodifiable(handle: i64) -> u8 {
    HEAP.with(|h| u8::from(h.borrow().imutaveis.contains(&handle)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_filled(len: i64, fill_bits: i64, fill_tag: u8) -> i64 {
    let count = usize::try_from(len.max(0)).expect("comprimento inválido");
    let val = tagged(fill_bits, fill_tag);
    let items = vec![val; count];
    HEAP.with(|heap| heap.borrow_mut().create_list(items))
}

// Construtores de Erros Core

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_new(msg_bits: i64, is_ref: u8) -> i64 {
    HEAP.with(|h| {
        h.borrow_mut().allocate(Value::Object {
            class_id: 1000,
            fields: if msg_bits == 0 && is_ref == 0 {
                Vec::new()
            } else {
                vec![(msg_bits, is_ref != 0)]
            },
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_format_exception_new(msg_handle: i64, src_handle: i64, offset: i64) -> i64 {
    HEAP.with(|h| {
        h.borrow_mut().allocate(Value::Object {
            class_id: 1001,
            fields: vec![(msg_handle, true), (src_handle, true), (offset, false)],
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_state_error_new(msg_handle: i64) -> i64 {
    alocar_erro_com_rastro(1002, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_argument_error_new(msg_handle: i64, name_handle: i64) -> i64 {
    alocar_erro_com_rastro(1003, vec![(msg_handle, true), (name_handle, true), (0, false), (0, false), (0, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_argument_error_value(val_bits: i64, val_is_ref: u8, name_handle: i64, msg_handle: i64) -> i64 {
    alocar_erro_com_rastro(1003, vec![(msg_handle, true), (name_handle, true), (val_bits, val_is_ref != 0), (1, false), (0, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_argument_error_not_null(name_handle: i64) -> i64 {
    alocar_erro_com_mensagem(
        1003,
        "Must not be null",
        Vec::new(),
        vec![(name_handle, true), (0, false), (0, false), (0, false)],
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_range_error_new(msg_handle: i64) -> i64 {
    alocar_erro_com_rastro(1004, vec![(msg_handle, true), (0, false), (0, false), (0, false), (0, false), (0, false), (0, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_range_error_value(val: i64, name_handle: i64, msg_handle: i64) -> i64 {
    alocar_erro_com_rastro(1004, vec![(msg_handle, true), (name_handle, true), (val, false), (0, false), (0, false), (0, false), (1, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_range_error_range(val: i64, min: i64, max: i64, name_handle: i64, msg_handle: i64) -> i64 {
    alocar_erro_com_rastro(1004, vec![(msg_handle, true), (name_handle, true), (val, false), (min, false), (max, false), (1, false), (1, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_range_error_index(index: i64, indexable_or_len: i64, name_handle: i64, msg_handle: i64) -> i64 {
    // O comprimento é lido e o empréstimo solto ANTES de pedir o rastro
    // (G6: a versão anterior chamava outra extern com `borrow_mut` aberto).
    let len = HEAP.with(|h| match h.borrow().try_get(indexable_or_len) {
        Some(Value::List(items)) => items.len() as i64,
        Some(Value::String(s)) => s.encode_utf16().count() as i64,
        _ => indexable_or_len,
    });
    alocar_erro_com_rastro(
        1004,
        vec![(msg_handle, true), (name_handle, true), (index, false), (0, false), (len, false), (2, false), (1, false)],
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unsupported_error_new(msg_handle: i64) -> i64 {
    alocar_erro_com_rastro(1005, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unimplemented_error_new(msg_handle: i64) -> i64 {
    alocar_erro_com_rastro(1008, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_assertion_error_new(msg_bits: i64, is_ref: u8) -> i64 {
    alocar_erro_com_rastro(1009, vec![(msg_bits, is_ref != 0)])
}

/// `ConcurrentModificationError([this.modifiedObject])`.
///
/// O campo 0 é o `modifiedObject` (handle, 0 = null) e o campo 1 é o rastro.
/// Guardar o objeto é o que permite ao `toString` produzir a forma longa da
/// VM; os chamadores (`List.add` durante `for-in`, `Map`/`Set`) passam a
/// coleção que estava sendo iterada, como o `dart:core` faz.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_concurrent_modification_error_new(modified: i64) -> i64 {
    alocar_erro_com_rastro(1010, vec![(modified, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_type_error_new() -> i64 {
    alocar_erro_com_rastro(1011, vec![(0, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_no_such_method_error_new(nome: i64) -> i64 {
    alocar_erro_com_rastro(1012, vec![(nome, true)])
}

// Getters de Erros Core

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_error_get_message(handle: i64) -> i64 {
    campo_como_ref(handle, 0)
}

/// Campo de um erro do runtime como referência (R5): um escalar guardado
/// (a mensagem de `AssertionError` pode ser um `int`) sai encaixotado; o
/// ausente, null.
fn campo_como_ref(handle: i64, indice: usize) -> i64 {
    let campo = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::Object { fields, .. } = heap.get(handle) else { return None; };
        fields.get(indice).copied()
    });
    match campo {
        Some((bits, true)) => bits,
        Some((0, false)) | None => 0,
        Some((bits, false)) => valor_como_ref(TaggedValue::scalar(bits)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_error_get_name(handle: i64) -> i64 {
    campo_como_ref(handle, 1)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_error_get_invalid_value(handle: i64) -> i64 {
    // `invalidValue` é `dynamic`: devolve referência (R5), encaixotando o
    // inteiro que o `RangeError` guarda como escalar.
    let campo = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::Object { fields, .. } = heap.get(handle) else { return None; };
        fields.get(2).copied()
    });
    match campo {
        Some((bits, true)) => bits,
        Some((bits, false)) => valor_como_ref(TaggedValue::scalar(bits)),
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_error_get_start(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::Object { fields, .. } = heap.get(handle) else { return 0; };
        fields.get(3).map_or(0, |(bits, _)| *bits)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_error_get_end(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::Object { fields, .. } = heap.get(handle) else { return 0; };
        fields.get(4).map_or(0, |(bits, _)| *bits)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_error_get_source(handle: i64) -> i64 {
    campo_como_ref(handle, 1)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_error_get_offset(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::Object { fields, .. } = heap.get(handle) else { return 0; };
        fields.get(2).map_or(0, |(bits, _)| if *bits < 0 { 0 } else { *bits })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_error_get_stack_trace(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::Object { class_id, fields } = heap.get(handle) else { return 0; };
        let cid = *class_id;
        let st_idx = match cid {
            1003 => 5,
            1004 => 7,
            _ => 1,
        };
        fields.get(st_idx).map_or(0, |(bits, _)| *bits)
    })
}

// Funções de Lista com checagem de erros

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_single(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if handle == 0 { return 0; }
        if let Value::List(items) = heap_ref.get(handle) {
            if items.is_empty() {
                drop(heap_ref);
                let err = allocate_state_error("No element");
                dartforge_exception_throw(err, 3);
                return 0;
            }
            if items.len() > 1 {
                drop(heap_ref);
                let err = allocate_state_error("Too many elements");
                dartforge_exception_throw(err, 3);
                return 0;
            }
            items[0].bits
        } else {
            0
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_sublist(handle: i64, start: i64, end: i64) -> i64 {
    HEAP.with(|heap| {
        let len = {
            let h = heap.borrow();
            match h.get(handle) {
                Value::List(items) => items.len() as i64,
                _ => 0,
            }
        };
        let end_idx = if end < 0 { len } else { end };
        if start < 0 || start > len || end_idx < start || end_idx > len {
            let err = allocate_range_error("Index out of range");
            dartforge_exception_throw(err, 3);
            return 0;
        }
        let h = heap.borrow();
        let Value::List(items) = h.get(handle) else { return 0; };
        let slice: Vec<TaggedValue> = items[start as usize..end_idx as usize].to_vec();
        drop(h);
        heap.borrow_mut().allocate(Value::List(slice))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_remove_at(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        let len = {
            let h = heap.borrow();
            match h.get(handle) {
                Value::List(items) => items.len() as i64,
                _ => 0,
            }
        };
        if index < 0 || index >= len {
            let err = allocate_range_error("Index out of range");
            dartforge_exception_throw(err, 3);
            return 0;
        }
        let mut h = heap.borrow_mut();
        let Value::List(items) = h.get_mut(handle) else { return 0; };
        items.remove(index as usize).bits
    })
}
