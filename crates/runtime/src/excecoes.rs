// Runtime nativo: exceção pendente, rastros e as classes de erro do SDK.

/// Uma string nova do heap com o texto `s` (`textos.rs`).
fn error_text(s: &str) -> i64 {
    HEAP.with(|h| h.borrow_mut().allocate_str(s))
}

/// O valor de uma exceção pendente numa posição `Ref` (encaixota o escalar
/// lançado: `throw 1` guarda o `int` sem caixa).
fn exception_as_ref(v: crate::heap::Value) -> i64 {
    HEAP.with(|h| h.borrow_mut().to_ref(v))
}

/// A exceção lançada pela ABI plana `(bits, tag)` do código gerado (1 `int`,
/// 2 `bool`, 3 `Ref`, 4 `double`).
fn abi_exception(bits: i64, tag: u8) -> crate::heap::Value {
    use crate::heap::Value;
    match tag {
        1 => Value::Int(bits),
        2 => Value::Bool(bits != 0),
        3 => Value::Ref(bits),
        4 => Value::Double(f64::from_bits(bits as u64)),
        _ => panic!("tag de valor inválida"),
    }
}

/// A etiqueta da ABI plana de uma exceção guardada.
fn exception_tag(v: crate::heap::Value) -> u8 {
    use crate::heap::Value;
    match v {
        Value::Int(_) => 1,
        Value::Bool(_) => 2,
        Value::Ref(_) => 3,
        Value::Double(_) => 4,
    }
}

/// Os bits da ABI plana de uma exceção guardada.
fn exception_bits(v: crate::heap::Value) -> i64 {
    use crate::heap::Value;
    match v {
        Value::Int(i) | Value::Ref(i) => i,
        Value::Bool(b) => i64::from(b),
        Value::Double(d) => d.to_bits() as i64,
    }
}

/// O `Ref` de uma exceção guardada que é referência (`None` para o escalar).
fn exception_ref(v: crate::heap::Value) -> Option<i64> {
    match v {
        crate::heap::Value::Ref(r) => Some(r),
        _ => None,
    }
}

/// A mensagem do `TypeError` de `x!` sobre null na VM.
const NULL_CHECK_MESSAGE: &str = "Null check operator used on a null value";

/// `x!` sobre null: o `TypeError` da VM, capturável, com a mensagem dela.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_null_check_error_new() -> i64 {
    if let Some(e) = source_error_with_text("_dartforgeErroDeTipo", NULL_CHECK_MESSAGE) {
        return e;
    }
    let m = error_text(NULL_CHECK_MESSAGE);
    with_roots(&[m], || allocate_error_with_trace(1011, vec![(m, true)]))
}

/// O texto do rastro da exceção corrente (o `StackTrace` do lançamento),
/// para a exceção não capturada; vazio sem rastro.
pub fn exception_trace_text() -> String {
    let Some(h) = CURRENT_STACK_TRACE.with(|slot| *slot.borrow()) else {
        // §13.14: sem `StackTrace` pedido, os endereços do lançamento (vazio
        // sem a tabela do rastro).
        let returns = throw_returns();
        return if returns.is_empty() { String::new() } else { symbolize_returns(&returns) };
    };
    trace_text(h).unwrap_or_default()
}

fn stack_trace_class_id() -> i64 {
    CLASS_NAMES.with(|map| {
        let map = map.borrow();
        map.iter().find(|(_, name)| *name == "_StackTrace")
            .or_else(|| map.iter().find(|(_, name)| *name == "StackTrace"))
            .map(|(&id, _)| id)
    }).unwrap_or(1006)
}

/// Objeto `StackTrace` com o texto dado (duas alocações, a primeira
/// enraizada durante a segunda).
fn allocate_stack_trace(text: &str) -> i64 {
    let trace_str = error_text(text);
    let cid = stack_trace_class_id();
    with_roots(&[trace_str], || {
        HEAP.with(|h| h.borrow_mut().new_object(cid, &[(trace_str, true)]))
    })
}

/// Objeto de erro com o rastro corrente no campo final (o `st` enraizado
/// enquanto o objeto é alocado).
fn allocate_error_with_trace(class_id: i64, mut fields: Vec<(i64, bool)>) -> i64 {
    let roots: Vec<i64> = fields.iter().filter(|(_, r)| *r).map(|(b, _)| *b).collect();
    with_roots(&roots, || {
        let st = dartforge_stack_trace_get();
        fields.push((st, true));
        with_roots(&[st], || HEAP.with(|h| h.borrow_mut().new_object(class_id, &fields)))
    })
}

/// Mensagem alocada, enraizada, e o erro com ela e o rastro.
fn allocate_error_with_message(class_id: i64, message: &str, before: Vec<(i64, bool)>, after: Vec<(i64, bool)>) -> i64 {
    let msg = error_text(message);
    with_roots(&[msg], || {
        let mut fields = before;
        fields.push((msg, true));
        fields.extend(after);
        allocate_error_with_trace(class_id, fields)
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
    // §13.14: os endereços do último `throw` viram o rastro corrente (o
    // `_stackTrace` do erro e o `catch (e, s)` veem o mesmo objeto), o texto
    // feito só no primeiro pedido.
    let returns = throw_returns();
    if !returns.is_empty() {
        let st = allocate_return_trace(&returns);
        CURRENT_STACK_TRACE.with(|slot| *slot.borrow_mut() = Some(st));
        HEAP.with(|h| h.borrow_mut().set_runtime_root(1, st));
        return st;
    }
    allocate_stack_trace(TRACE_WITHOUT_TABLE)
}

/// Native do getter estático `StackTrace.current` da VM: com a tabela do
/// rastro, os endereços desta pilha (§13.14).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StackTrace_current() -> i64 {
    let returns = capture_returns();
    if !returns.is_empty() {
        return allocate_return_trace(&returns);
    }
    dartforge_stack_trace_get()
}

/// Lança exceção associando um stack trace explícito.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_throw_with_stack_trace(bits: i64, tag: u8, st_handle: i64) {
    CURRENT_STACK_TRACE.with(|slot| *slot.borrow_mut() = Some(st_handle));
    HEAP.with(|h| h.borrow_mut().set_runtime_root(1, st_handle));
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
///
/// O valor é um `heap::Value` (fora do heap): o escalar lançado fica sem
/// caixa; `nucleo.rs` o lê no fim do programa.
thread_local! {
    static EXCEPTION: RefCell<Option<crate::heap::Value>> = RefCell::new(None);
    /// O isolado está sendo desenrolado para terminar (`Isolate.exit`, o
    /// `UnwindError` da VM): a exceção pendente não é capturável, nenhum
    /// `catch` a recebe e ela não sai da pendência até o laço de eventos.
    static UNWINDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Grava a exceção pendente e o espelho dela no contexto da thread (que o
/// código gerado lê, `Context::pending`).
fn definir_excecao(v: Option<crate::heap::Value>) {
    CONTEXTO.with(|c| c.pending.set(u8::from(v.is_some())));
    EXCEPTION.with(|slot| *slot.borrow_mut() = v);
}

/// Toma a exceção pendente (e limpa o espelho).
fn take_exception() -> Option<crate::heap::Value> {
    CONTEXTO.with(|c| c.pending.set(0));
    EXCEPTION.with(|slot| slot.borrow_mut().take())
}

/// Começa a desenrolar o isolado: uma exceção pendente que nenhum `catch`
/// recebe, até o laço de eventos (`isolados.rs`).
fn begin_unwind() {
    UNWINDING.with(|d| d.set(true));
    definir_excecao(Some(crate::heap::Value::Ref(0)));
    HEAP.with(|h| h.borrow_mut().set_runtime_root(0, 0));
}

/// Se o isolado está sendo desenrolado para terminar.
fn unwinding_now() -> bool {
    UNWINDING.with(|d| d.get())
}

/// Se a exceção pendente pode ser capturada por um `catch` (o tratador
/// gerado pergunta antes dos testes `on T`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_capturavel() -> u8 {
    u8::from(!unwinding_now())
}

/// A exceção pendente como referência (o valor da variável do `catch`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_peek_ref() -> i64 {
    match EXCEPTION.with(|slot| *slot.borrow()) {
        Some(v) => exception_as_ref(v),
        None => 0,
    }
}

fn allocate_state_error(message: &str) -> i64 {
    allocate_error_with_message(1002, message, Vec::new(), Vec::new()) // StateError
}

fn allocate_range_error(message: &str) -> i64 {
    allocate_error_with_message(1004, message, Vec::new(), Vec::new()) // RangeError
}

/// Registra a exceção pendente; referências devem estar vivas e enraizadas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_throw(bits: i64, tag: u8) {
    // §13.14: os endereços de retorno do lançamento, antes de qualquer
    // alocação (o rastro explícito de `Error.throwWithStackTrace` já está no
    // rastro corrente).
    if CURRENT_STACK_TRACE.with(|slot| slot.borrow().is_none()) {
        store_throw_returns();
    }
    let value = abi_exception(bits, tag);
    let reference = exception_ref(value).unwrap_or(0);
    if reference != 0 {
        // O SDK da fonte reserva o primeiro campo de `Error` para
        // `_stackTrace`; subclasses conservam esse prefixo no layout R7.
        // Os erros internos (ids 1000–1012) mantêm seus índices próprios.
        // O rastro é alocado SEM empréstimo mutável do heap aberto (G6).
        let sdk_error = CLASS_NAMES.with(|map| {
            map.borrow().iter().find(|(_, name)| *name == "Error").map(|(&id, _)| id)
        });
        let needs = HEAP.with(|heap| match heap.borrow().object(reference) {
            Some(fields) if (1000..=1012).contains(&fields.class_id) && dartforge_is_subclass(fields.class_id, 1007) != 0 => {
                let st_idx = match fields.class_id {
                    1003 => 5,
                    1004 => 7,
                    _ => 1,
                };
                // Sem o campo do rastro, nada (o corpo de fora só nasce na
                // recarga do JIT: o código de produção lê o corpo uma vez por
                // objeto, `llvm/mod.rs`, `ajudante_do_corpo`).
                fields.get(st_idx).is_some_and(|f| f.0 == 0).then_some(st_idx)
            }
            Some(fields) if sdk_error.is_some_and(|cid| dartforge_is_subclass(fields.class_id, cid) != 0) => {
                fields.first().is_some_and(|(value_ref, _)| value_ref == 0).then_some(0)
            }
            _ => None,
        });
        if let Some(st_idx) = needs {
            // O rastro aloca: o erro fica enraizado até virar a exceção
            // pendente (quem lança nem sempre o enraizou).
            let st = with_roots(&[reference], || dartforge_stack_trace_get());
            HEAP.with(|heap| {
                let mut heap = heap.borrow_mut();
                heap.ensure_fields(reference, st_idx + 1);
                heap.set_field(reference, st_idx, st, true);
            });
        }
    }
    if debug() {
        let cid = if exception_ref(value).is_some() { dartforge_value_class(reference) } else { -100 };
        let name = CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned()).unwrap_or_default();
        eprintln!("[depurar] throw: classe {cid} {name}");
        show_trace();
    }
    // G6: a exceção pendente é raiz até ser consumida.
    HEAP.with(|h| h.borrow_mut().set_runtime_root(0, reference));
    definir_excecao(Some(value));
}

/// Indica se há exceção pendente na thread corrente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_pending() -> u8 {
    CONTEXTO.with(|c| c.pending.get())
}

/// Desarma e limpa o slot de exceção pendente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_clear() {
    if unwinding_now() {
        // O desenrolar não sai da pendência (`finally`, `return` e saltos
        // também limpam): a pendência chega ao laço de eventos.
        return;
    }
    // Sem exceção nem rastro guardado (o caso de todo `return`): nada a
    // limpar, e o heap não é tocado.
    if CONTEXTO.with(|c| c.pending.get()) == 0 && CURRENT_STACK_TRACE.with(|slot| slot.borrow().is_none()) {
        return;
    }
    take_exception();
    CURRENT_STACK_TRACE.with(|slot| {
        slot.borrow_mut().take();
    });
    forget_throw_returns();
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.set_runtime_root(0, 0);
        h.set_runtime_root(1, 0);
    });
}

fn allocate_format_exception(message: &str) -> i64 {
    let msg = error_text(message);
    if ajudante("_dartforgeErroDeFormato").is_some() {
        return with_roots(&[msg], || dartforge_format_exception_new(msg, 0, -1));
    }
    with_roots(&[msg], || {
        HEAP.with(|h| {
            // FormatException
            h.borrow_mut().new_object(1001, &[(msg, true), (0, true), (-1, false)])
        })
    })
}

// Construtores de Erros Core

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_new(msg_bits: i64, is_ref: u8) -> i64 {
    HEAP.with(|h| {
        let fields: &[crate::heap::Field] = if msg_bits == 0 && is_ref == 0 { &[] } else { &[(msg_bits, is_ref != 0)] };
        h.borrow_mut().new_object(1000, fields)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_format_exception_new(msg_handle: i64, src_handle: i64, offset: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroDeFormato") {
        // SAFETY: registrado pelo `dart:core` com a assinatura
        // `(String, Object?, int) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64, a1: i64, a2: i64| -> i64 { dart_r3(dart_target, a0, a1, a2) } };
        return with_roots(&[msg_handle, src_handle], || g(msg_handle, src_handle, offset));
    }
    HEAP.with(|h| {
        h.borrow_mut().new_object(1001, &[(msg_handle, true), (src_handle, true), (offset, false)])
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_state_error_new(msg_handle: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroDeEstado") {
        // SAFETY: registrado pelo `dart:core` com a assinatura `(String) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        return with_roots(&[msg_handle], || g(msg_handle));
    }
    allocate_error_with_trace(1002, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_argument_error_new(msg_handle: i64, name_handle: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroDeArgumento") {
        // SAFETY: registrado pelo `dart:core` com a assinatura
        // `(String?, String?) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64, a1: i64| -> i64 { dart_r2(dart_target, a0, a1) } };
        return with_roots(&[msg_handle, name_handle], || g(msg_handle, name_handle));
    }
    allocate_error_with_trace(1003, vec![(msg_handle, true), (name_handle, true), (0, false), (0, false), (0, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_argument_error_value(val_bits: i64, val_is_ref: u8, name_handle: i64, msg_handle: i64) -> i64 {
    allocate_error_with_trace(1003, vec![(msg_handle, true), (name_handle, true), (val_bits, val_is_ref != 0), (1, false), (0, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_argument_error_not_null(name_handle: i64) -> i64 {
    allocate_error_with_message(
        1003,
        "Must not be null",
        Vec::new(),
        vec![(name_handle, true), (0, false), (0, false), (0, false)],
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_range_error_new(msg_handle: i64) -> i64 {
    allocate_error_with_trace(1004, vec![(msg_handle, true), (0, false), (0, false), (0, false), (0, false), (0, false), (0, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_range_error_range(val: i64, min: i64, max: i64, name_handle: i64, msg_handle: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroDeFaixa") {
        // SAFETY: registrado pelo `dart:core`: `(int, int, int, String?) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64, a1: i64, a2: i64, a3: i64| -> i64 { dart_r4(dart_target, a0, a1, a2, a3) } };
        let _ = msg_handle;
        return with_roots(&[name_handle], || g(val, min, max, name_handle));
    }
    allocate_error_with_trace(1004, vec![(msg_handle, true), (name_handle, true), (val, false), (min, false), (max, false), (1, false), (1, false)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_range_error_index(index: i64, indexable_or_len: i64, name_handle: i64, msg_handle: i64) -> i64 {
    // O comprimento é lido e o empréstimo solto ANTES de pedir o rastro
    // (G6: a versão anterior chamava outra extern com `borrow_mut` aberto).
    // `indexable_or_len` é uma lista, uma string ou o próprio comprimento
    // (um `int` cru, que só é lido como objeto se for um bloco vivo).
    let len = HEAP.with(|h| {
        let h = h.borrow();
        let x = indexable_or_len;
        if !crate::layout::is_object(x) || !h.is_live_object(x) {
            x
        } else if h.e_lista(x) {
            h.list_len(x) as i64
        } else {
            h.text(x).map_or(x, |t| t.len() as i64)
        }
    });
    allocate_error_with_trace(
        1004,
        vec![(msg_handle, true), (name_handle, true), (index, false), (0, false), (len, false), (2, false), (1, false)],
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unsupported_error_new(msg_handle: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroNaoSuportado") {
        // SAFETY: registrado pelo `dart:core` com a assinatura `(String?) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        return with_roots(&[msg_handle], || g(msg_handle));
    }
    allocate_error_with_trace(1005, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unimplemented_error_new(msg_handle: i64) -> i64 {
    allocate_error_with_trace(1008, vec![(msg_handle, true)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_assertion_error_new(msg_bits: i64, is_ref: u8) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroDeAssercao") {
        // SAFETY: registrado pelo `dart:core` com a assinatura `(Object?) -> Object`.
        let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        let m = if is_ref != 0 { msg_bits } else { HEAP.with(|h| h.borrow_mut().to_ref(crate::heap::Value::Int(msg_bits))) };
        return with_roots(&[m], || g(m));
    }
    allocate_error_with_trace(1009, vec![(msg_bits, is_ref != 0)])
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_type_error_new() -> i64 {
    // SDK da fonte: o `_TypeError` da fonte (`identical_patch.dart`).
    if let Some(e) = source_error_with_text("_dartforgeErroDeTipo", "TypeError") {
        return e;
    }
    allocate_error_with_trace(1011, vec![(0, false)])
}

/// Erro de leitura/escrita de `late`: 0/1 = campo, 2/3 = local,
/// 4/5 = escrita durante o inicializador. A fonte do SDK constrói `LateError`
/// para preservar a identidade `is Error` e o `toString` da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_error_new(name: i64, code: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroLate") {
        // SAFETY: helper registrado pelo dart:_internal como (String, int) -> Object.
        let g = { let dart_target: usize = f; move |a0: i64, a1: i64| -> i64 { dart_r2(dart_target, a0, a1) } };
        return with_roots(&[name], || g(name, code));
    }
    let n = HEAP.with(|h| h.borrow().text(name).map(|t| t.para_string()).unwrap_or_default());
    let (location, message) = match code {
        0 => ("Field", "has not been initialized"),
        1 => ("Field", "has already been initialized"),
        2 => ("Local", "has not been initialized"),
        3 => ("Local", "has already been initialized"),
        4 => ("Field", "has been assigned during initialization"),
        _ => ("Local", "has been assigned during initialization"),
    };
    // No modo sem SDK da fonte, ao menos lança um Error capturável. Não
    // reservamos um CID sintético: 1013 pode pertencer a uma classe real.
    let text = format!("LateInitializationError: {location} '{n}' {message}.");
    let msg = error_text(&text);
    dartforge_state_error_new(msg)
}

/// Reentrância de um inicializador global: a VM recursaria no getter até
/// lançar `StackOverflowError`. Construímos o mesmo erro sem consumir a pilha.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_stack_overflow_error_new() -> i64 {
    if let Some(f) = ajudante("_dartforgeErroPilha") {
        // SAFETY: helper registrado pelo dart:_internal como () -> Object.
        let g = { let dart_target: usize = f; move || -> i64 { dart_r0(dart_target) } };
        return g();
    }
    let msg = error_text("Stack Overflow");
    dartforge_state_error_new(msg)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_no_such_method_error_new(name: i64) -> i64 {
    if debug() {
        let t = HEAP.with(|h| h.borrow().text(name).map(|t| t.para_string()));
        eprintln!("[depurar] NoSuchMethodError: {t:?}");
    }
    // No SDK da fonte o erro é sempre o `NoSuchMethodError` do SDK (a
    // classe sintética 1012 não é subtipo de nada no RTI da biblioteca
    // compilada: nem um `catch (e)` a pegaria).
    if let Some(f) = ajudante("_dartforgeErroDeChamada") {
        // O SDK da fonte fornece a instância concreta de NoSuchMethodError.
        // O nome é enraizado enquanto os construtores Dart alocam.
        let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        return with_roots(&[name], || g(name));
    }
    allocate_error_with_trace(1012, vec![(name, true)])
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

/// Campo de um erro do runtime como referência (R5): um escalar guardado
/// (a mensagem de `AssertionError` pode ser um `int`) sai encaixotado; o
/// ausente, null.
fn field_as_ref(handle: i64, ix: usize) -> i64 {
    let field = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Some(fields) = heap.object(handle) else { return None; };
        fields.get(ix)
    });
    match field {
        Some((bits, true)) => bits,
        Some((0, false)) | None => 0,
        Some((bits, false)) => HEAP.with(|h| h.borrow_mut().to_ref(crate::heap::Value::Int(bits))),
    }
}


/// SDK da fonte: o erro construído pela função Dart `name(text)`
/// registrada (`identical_patch.dart`), ou `None` sem ela.
fn source_error_with_text(name: &str, text: &str) -> Option<i64> {
    let f = ajudante(name)?;
    // SAFETY: registrado pelo `dart:core` com a assinatura `(String) -> Object`.
    let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
    let t = error_text(text);
    Some(with_roots(&[t], || g(t)))
}

// Os construtores de erro sob ARC: o objeto novo com um owner do código. Os
// argumentos `Ref` ficam emprestados; o construtor pode rodar Dart (dentro de
// um escopo nativo) e deixar exceção pendente, caso em que o resultado é null
// sem owner.

/// `Exception(msg)` sob ARC, com owner do código ([`dartforge_exception_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_exception_new_owned_v1(msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_exception_new(msg, 1))
}

/// `AssertionError(msg)` sob ARC, com owner do código ([`dartforge_assertion_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_assertion_error_new_owned_v1(msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_assertion_error_new(msg, 1))
}

/// `StateError(msg)` sob ARC, com owner do código ([`dartforge_state_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_state_error_new_owned_v1(msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_state_error_new(msg))
}

/// `UnsupportedError(msg)` sob ARC, com owner do código ([`dartforge_unsupported_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_unsupported_error_new_owned_v1(msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_unsupported_error_new(msg))
}

/// `UnimplementedError(msg)` sob ARC, com owner do código ([`dartforge_unimplemented_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_unimplemented_error_new_owned_v1(msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_unimplemented_error_new(msg))
}

/// `ArgumentError(msg, name)` sob ARC, com owner do código ([`dartforge_argument_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_argument_error_new_owned_v1(msg: i64, name: i64) -> i64 {
    owner_in_native_scope(|| dartforge_argument_error_new(msg, name))
}

/// `ArgumentError.value(value_ref, name, msg)` sob ARC, com owner do código ([`dartforge_argument_error_value`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_argument_error_value_owned_v1(value_ref: i64, name: i64, msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_argument_error_value(value_ref, 1, name, msg))
}

/// `ArgumentError.notNull(name)` sob ARC, com owner do código ([`dartforge_argument_error_not_null`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_argument_error_not_null_owned_v1(name: i64) -> i64 {
    owner_in_native_scope(|| dartforge_argument_error_not_null(name))
}

/// `RangeError.range(...)` sob ARC, com owner do código ([`dartforge_range_error_range`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_range_error_range_owned_v1(value_ref: i64, min: i64, max: i64, name: i64, msg: i64) -> i64 {
    owner_in_native_scope(|| dartforge_range_error_range(value_ref, min, max, name, msg))
}

/// o `TypeError` do runtime sob ARC, com owner do código ([`dartforge_type_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_type_error_new_owned_v1() -> i64 {
    owner_in_native_scope(|| dartforge_type_error_new())
}

/// o erro do `!` sobre null sob ARC, com owner do código ([`dartforge_null_check_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_null_check_error_new_owned_v1() -> i64 {
    owner_in_native_scope(|| dartforge_null_check_error_new())
}

/// o `LateError` do campo/local `name` sob ARC, com owner do código ([`dartforge_late_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_late_error_new_owned_v1(name: i64, code: i64) -> i64 {
    owner_in_native_scope(|| dartforge_late_error_new(name, code))
}

/// o `StackOverflowError` da reentrada sob ARC, com owner do código ([`dartforge_stack_overflow_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_stack_overflow_error_new_owned_v1() -> i64 {
    owner_in_native_scope(|| dartforge_stack_overflow_error_new())
}

/// o `NoSuchMethodError` de `name` sob ARC, com owner do código ([`dartforge_no_such_method_error_new`]).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_no_such_method_error_new_owned_v1(name: i64) -> i64 {
    owner_in_native_scope(|| dartforge_no_such_method_error_new(name))
}
