// Runtime nativo: exceção pendente, rastros e as classes de erro do SDK.

/// Encerra o processo quando uma asserção de não nulidade falha.
///
/// Ainda não há exceções Dart capturáveis; a falha é explícita e não retorna.
// SAFETY: símbolo reservado e contrato C sem retorno, conforme a declaração LLVM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_null_assert_fail() -> ! {
    // Com o SDK da fonte o `!` sobre null também encerra aqui (o lowering
    // não tem como continuar depois desta chamada); o texto é o da VM.
    use std::io::Write;
    let _ = writeln!(
        std::io::stderr().lock(),
        "Null check operator used on a null value"
    );
    std::process::exit(101)
}

fn id_da_classe_stack_trace() -> i64 {
    CLASS_NAMES.with(|map| {
        let map = map.borrow();
        map.iter().find(|(_, name)| *name == "_StackTrace")
            .or_else(|| map.iter().find(|(_, name)| *name == "StackTrace"))
            .map(|(&id, _)| id)
    }).unwrap_or(1006)
}

/// Objeto `StackTrace` com o texto dado (duas alocações, a primeira
/// enraizada durante a segunda).
fn alocar_stack_trace(texto: &str) -> i64 {
    let trace_str = HEAP.with(|h| h.borrow_mut().allocate(Value::String(Texto::de_str(texto))));
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
    let msg = HEAP.with(|h| h.borrow_mut().allocate(Value::String(Texto::de_str(mensagem))));
    com_raizes(&[msg], || {
        let mut campos = antes;
        campos.push((msg, true));
        campos.extend(depois);
        alocar_erro_com_rastro(class_id, campos)
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

/// Native do getter estático `StackTrace.current` da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StackTrace_current() -> i64 {
    dartforge_stack_trace_get()
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

/// Native da VM `Error._throw`: instala o rastro explícito e entrega a
/// exceção ao protocolo de exceção pendente do código gerado. O retorno é
/// inalcançável em Dart (`Never`), mas ocupa `Ref` na ABI do SDK da fonte.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Error_throwWithStackTrace(error: i64, trace: i64) -> i64 {
    dartforge_throw_with_stack_trace(error, 3, trace);
    0
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
    /// O isolado está sendo desenrolado para terminar (`Isolate.exit`, o
    /// `UnwindError` da VM): a exceção pendente não é capturável, nenhum
    /// `catch` a recebe e ela não sai da pendência até o laço de eventos.
    static DESENROLANDO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Começa a desenrolar o isolado: uma exceção pendente que nenhum `catch`
/// recebe, até o laço de eventos (`isolados.rs`).
fn comecar_desenrolar() {
    DESENROLANDO.with(|d| d.set(true));
    EXCEPTION.with(|slot| *slot.borrow_mut() = Some(TaggedValue::reference(0)));
    HEAP.with(|h| h.borrow_mut().set_raiz_do_runtime(0, 0));
}

/// Se o isolado está sendo desenrolado para terminar.
fn desenrolando() -> bool {
    DESENROLANDO.with(|d| d.get())
}

/// Se a exceção pendente pode ser capturada por um `catch` (o tratador
/// gerado pergunta antes dos testes `on T`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_capturavel() -> u8 {
    u8::from(!desenrolando())
}

/// A exceção pendente como referência (o valor da variável do `catch`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_peek_ref() -> i64 {
    match EXCEPTION.with(|slot| *slot.borrow()) {
        Some(v) => valor_como_ref(v),
        None => 0,
    }
}

fn allocate_state_error(message: &str) -> i64 {
    alocar_erro_com_mensagem(1002, message, Vec::new(), Vec::new()) // StateError
}

fn allocate_range_error(message: &str) -> i64 {
    alocar_erro_com_mensagem(1004, message, Vec::new(), Vec::new()) // RangeError
}

/// Registra a exceção pendente; referências devem estar vivas e enraizadas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_throw(bits: i64, tag: u8) {
    let value = tagged(bits, tag);
    if value.is_ref && value.bits != 0 {
        // O SDK da fonte reserva o primeiro campo de `Error` para
        // `_stackTrace`; subclasses conservam esse prefixo no layout R7.
        // Os erros internos (ids 1000–1012) mantêm seus índices próprios.
        // O rastro é alocado SEM empréstimo mutável do heap aberto (G6).
        let erro_sdk = CLASS_NAMES.with(|map| {
            map.borrow().iter().find(|(_, nome)| *nome == "Error").map(|(&id, _)| id)
        });
        let precisa = HEAP.with(|heap| match heap.borrow().try_get(value.bits) {
            Some(Value::Object { class_id, fields }) if (1000..=1012).contains(class_id) && dartforge_is_subclass(*class_id, 1007) != 0 => {
                let st_idx = match *class_id {
                    1003 => 5,
                    1004 => 7,
                    _ => 1,
                };
                (fields.get(st_idx).map_or(0, |f| f.0) == 0).then_some(st_idx)
            }
            Some(Value::Object { class_id, fields }) if erro_sdk.is_some_and(|cid| dartforge_is_subclass(*class_id, cid) != 0) => {
                fields.first().is_some_and(|(valor, _)| *valor == 0).then_some(0)
            }
            _ => None,
        });
        if let Some(st_idx) = precisa {
            // O rastro aloca: o erro fica enraizado até virar a exceção
            // pendente (quem lança nem sempre o enraizou).
            let st = com_raizes(&[value.bits], || dartforge_stack_trace_get());
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
    if depurar() {
        let cid = if value.is_ref { dartforge_value_class(value.bits) } else { -100 };
        let nome = CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned()).unwrap_or_default();
        eprintln!("[depurar] throw: classe {cid} {nome}");
        mostrar_rastro();
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
    if desenrolando() {
        return 0;
    }
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
    if desenrolando() {
        // O desenrolar não sai da pendência (`finally`, `return` e saltos
        // também limpam): a pendência chega ao laço de eventos.
        return;
    }
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

fn allocate_format_exception(message: &str) -> i64 {
    let msg = HEAP.with(|h| h.borrow_mut().allocate(Value::String(Texto::de_str(message))));
    com_raizes(&[msg], || {
        HEAP.with(|h| {
            h.borrow_mut().allocate(Value::Object {
                class_id: 1001, // FormatException
                fields: vec![(msg, true), (0, true), (-1, false)],
            })
        })
    })
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
    if let Some(f) = ajudante("_dartforgeErroDeEstado") {
        // SAFETY: registrado pelo `dart:core` com a assinatura `(String) -> Object`.
        let g: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(f) };
        return com_raizes(&[msg_handle], || g(msg_handle));
    }
    alocar_erro_com_rastro(1002, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_argument_error_new(msg_handle: i64, name_handle: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroDeArgumento") {
        // SAFETY: registrado pelo `dart:core` com a assinatura
        // `(String?, String?) -> Object`.
        let g: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
        return com_raizes(&[msg_handle, name_handle], || g(msg_handle, name_handle));
    }
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
    if let Some(f) = ajudante("_dartforgeErroDeFaixa") {
        // SAFETY: registrado pelo `dart:core`: `(int, int, int, String?) -> Object`.
        let g: extern "C" fn(i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
        let _ = msg_handle;
        return com_raizes(&[name_handle], || g(val, min, max, name_handle));
    }
    alocar_erro_com_rastro(1004, vec![(msg_handle, true), (name_handle, true), (val, false), (min, false), (max, false), (1, false), (1, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_range_error_index(index: i64, indexable_or_len: i64, name_handle: i64, msg_handle: i64) -> i64 {
    // O comprimento é lido e o empréstimo solto ANTES de pedir o rastro
    // (G6: a versão anterior chamava outra extern com `borrow_mut` aberto).
    let len = HEAP.with(|h| match h.borrow().try_get(indexable_or_len) {
        Some(Value::List(items)) => items.len() as i64,
        Some(Value::String(s)) => s.len() as i64,
        _ => indexable_or_len,
    });
    alocar_erro_com_rastro(
        1004,
        vec![(msg_handle, true), (name_handle, true), (index, false), (0, false), (len, false), (2, false), (1, false)],
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unsupported_error_new(msg_handle: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroNaoSuportado") {
        // SAFETY: registrado pelo `dart:core` com a assinatura `(String?) -> Object`.
        let g: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(f) };
        return com_raizes(&[msg_handle], || g(msg_handle));
    }
    alocar_erro_com_rastro(1005, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unimplemented_error_new(msg_handle: i64) -> i64 {
    alocar_erro_com_rastro(1008, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_assertion_error_new(msg_bits: i64, is_ref: u8) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroDeAssercao") {
        // SAFETY: registrado pelo `dart:core` com a assinatura `(Object?) -> Object`.
        let g: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(f) };
        let m = if is_ref != 0 { msg_bits } else { valor_como_ref(TaggedValue::scalar(msg_bits)) };
        return com_raizes(&[m], || g(m));
    }
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
    // SDK da fonte: o `_TypeError` da fonte (`identical_patch.dart`).
    if let Some(e) = erro_da_fonte_com_texto("_dartforgeErroDeTipo", "TypeError") {
        return e;
    }
    alocar_erro_com_rastro(1011, vec![(0, false)])
}

/// Erro de leitura/escrita de `late`: 0/1 = campo, 2/3 = local,
/// 4/5 = escrita durante o inicializador. A fonte do SDK constrói `LateError`
/// para preservar a identidade `is Error` e o `toString` da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_error_new(nome: i64, codigo: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroLate") {
        // SAFETY: helper registrado pelo dart:_internal como (String, int) -> Object.
        let g: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
        return com_raizes(&[nome], || g(nome, codigo));
    }
    let n = HEAP.with(|h| h.borrow().texto(nome).para_string());
    let (onde, mensagem) = match codigo {
        0 => ("Field", "has not been initialized"),
        1 => ("Field", "has already been initialized"),
        2 => ("Local", "has not been initialized"),
        3 => ("Local", "has already been initialized"),
        4 => ("Field", "has been assigned during initialization"),
        _ => ("Local", "has been assigned during initialization"),
    };
    // No modo sem SDK da fonte, ao menos lança um Error capturável. Não
    // reservamos um CID sintético: 1013 pode pertencer a uma classe real.
    let texto = format!("LateInitializationError: {onde} '{n}' {mensagem}.");
    let msg = HEAP.with(|h| h.borrow_mut().allocate(Value::String(Texto::de_str(&texto))));
    dartforge_state_error_new(msg)
}

/// Reentrância de um inicializador global: a VM recursaria no getter até
/// lançar `StackOverflowError`. Construímos o mesmo erro sem consumir a pilha.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_stack_overflow_error_new() -> i64 {
    if let Some(f) = ajudante("_dartforgeErroPilha") {
        // SAFETY: helper registrado pelo dart:_internal como () -> Object.
        let g: extern "C" fn() -> i64 = unsafe { std::mem::transmute(f) };
        return g();
    }
    let msg = HEAP.with(|h| h.borrow_mut().allocate(Value::String(Texto::de_str("Stack Overflow"))));
    dartforge_state_error_new(msg)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_no_such_method_error_new(nome: i64) -> i64 {
    if depurar() {
        let t = HEAP.with(|h| h.borrow().try_get(nome).map(|_| h.borrow().texto(nome).para_string()));
        eprintln!("[depurar] NoSuchMethodError: {t:?}");
    }
    // No SDK da fonte o erro é sempre o `NoSuchMethodError` do SDK (a
    // classe sintética 1012 não é subtipo de nada no RTI da biblioteca
    // compilada: nem um `catch (e)` a pegaria).
    if let Some(f) = ajudante("_dartforgeErroDeChamada") {
        // O SDK da fonte fornece a instância concreta de NoSuchMethodError.
        // O nome é enraizado enquanto os construtores Dart alocam.
        let g: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(f) };
        return com_raizes(&[nome], || g(nome));
    }
    alocar_erro_com_rastro(1012, vec![(nome, true)])
}

/// A consulta de assinatura é usada apenas na formatação detalhada da VM.
/// Para uma invocação construída por `Invocation.method`, o `toString` do SDK
/// segue `_toStringPlain` e não consulta este native; null é o contrato de
/// "sem assinatura encontrada" nos demais casos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_NoSuchMethodError_existingMethodSignature(
    _receiver: i64,
    _method_name: i64,
    _invocation_type: i64,
) -> i64 {
    0
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


/// SDK da fonte: o erro construído pela função Dart `nome(texto)`
/// registrada (`identical_patch.dart`), ou `None` sem ela.
fn erro_da_fonte_com_texto(nome: &str, texto: &str) -> Option<i64> {
    let f = ajudante(nome)?;
    // SAFETY: registrado pelo `dart:core` com a assinatura `(String) -> Object`.
    let g: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(f) };
    let t = HEAP.with(|h| h.borrow_mut().allocate(Value::String(Texto::de_str(texto))));
    Some(com_raizes(&[t], || g(t)))
}
