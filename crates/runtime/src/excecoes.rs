// Runtime nativo: exceção pendente, rastros e as classes de erro do SDK.

/// Uma string nova do heap com o texto `s` (`textos.rs`).
fn texto_de_erro(s: &str) -> i64 {
    HEAP.with(|h| h.borrow_mut().alocar_str(s))
}

/// O valor de uma exceção pendente numa posição `Ref` (encaixota o escalar
/// lançado: `throw 1` guarda o `int` sem caixa).
fn excecao_como_ref(v: crate::heap::Valor) -> i64 {
    HEAP.with(|h| h.borrow_mut().como_ref(v))
}

/// A exceção lançada pela ABI plana `(bits, tag)` do código gerado (1 `int`,
/// 2 `bool`, 3 `Ref`, 4 `double`).
fn excecao_da_abi(bits: i64, tag: u8) -> crate::heap::Valor {
    use crate::heap::Valor;
    match tag {
        1 => Valor::Int(bits),
        2 => Valor::Bool(bits != 0),
        3 => Valor::Ref(bits),
        4 => Valor::Double(f64::from_bits(bits as u64)),
        _ => panic!("tag de valor inválida"),
    }
}

/// A etiqueta da ABI plana de uma exceção guardada.
fn etiqueta_da_excecao(v: crate::heap::Valor) -> u8 {
    use crate::heap::Valor;
    match v {
        Valor::Int(_) => 1,
        Valor::Bool(_) => 2,
        Valor::Ref(_) => 3,
        Valor::Double(_) => 4,
    }
}

/// Os bits da ABI plana de uma exceção guardada.
fn bits_da_excecao(v: crate::heap::Valor) -> i64 {
    use crate::heap::Valor;
    match v {
        Valor::Int(i) | Valor::Ref(i) => i,
        Valor::Bool(b) => i64::from(b),
        Valor::Double(d) => d.to_bits() as i64,
    }
}

/// O `Ref` de uma exceção guardada que é referência (`None` para o escalar).
fn ref_da_excecao(v: crate::heap::Valor) -> Option<i64> {
    match v {
        crate::heap::Valor::Ref(r) => Some(r),
        _ => None,
    }
}

/// A mensagem do `TypeError` de `x!` sobre null na VM.
const MENSAGEM_DE_NULL_CHECK: &str = "Null check operator used on a null value";

/// `x!` sobre null: o `TypeError` da VM, capturável, com a mensagem dela.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_null_check_error_new() -> i64 {
    if let Some(e) = erro_da_fonte_com_texto("_dartforgeErroDeTipo", MENSAGEM_DE_NULL_CHECK) {
        return e;
    }
    let m = texto_de_erro(MENSAGEM_DE_NULL_CHECK);
    com_raizes(&[m], || alocar_erro_com_rastro(1011, vec![(m, true)]))
}

/// O texto do rastro da exceção corrente (o `StackTrace` do lançamento),
/// para a exceção não capturada; vazio sem rastro.
pub fn texto_do_rastro_da_excecao() -> String {
    let Some(h) = CURRENT_STACK_TRACE.with(|slot| *slot.borrow()) else { return String::new() };
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Some(campo) = heap.objeto(h).and_then(|o| o.first()) else { return String::new() };
        match campo {
            (t, true) => heap.texto(t).map(|t| t.para_string()).unwrap_or_default(),
            _ => String::new(),
        }
    })
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
    let trace_str = texto_de_erro(texto);
    let cid = id_da_classe_stack_trace();
    com_raizes(&[trace_str], || {
        HEAP.with(|h| h.borrow_mut().novo_objeto(cid, &[(trace_str, true)]))
    })
}

/// Objeto de erro com o rastro corrente no campo final (o `st` enraizado
/// enquanto o objeto é alocado).
fn alocar_erro_com_rastro(class_id: i64, mut campos: Vec<(i64, bool)>) -> i64 {
    let raizes: Vec<i64> = campos.iter().filter(|(_, r)| *r).map(|(b, _)| *b).collect();
    com_raizes(&raizes, || {
        let st = dartforge_stack_trace_get();
        campos.push((st, true));
        com_raizes(&[st], || HEAP.with(|h| h.borrow_mut().novo_objeto(class_id, &campos)))
    })
}

/// Mensagem alocada, enraizada, e o erro com ela e o rastro.
fn alocar_erro_com_mensagem(class_id: i64, mensagem: &str, antes: Vec<(i64, bool)>, depois: Vec<(i64, bool)>) -> i64 {
    let msg = texto_de_erro(mensagem);
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
///
/// O valor é um `heap::Valor` (fora do heap): o escalar lançado fica sem
/// caixa; `nucleo.rs` o lê no fim do programa.
thread_local! {
    static EXCEPTION: RefCell<Option<crate::heap::Valor>> = RefCell::new(None);
    /// O isolado está sendo desenrolado para terminar (`Isolate.exit`, o
    /// `UnwindError` da VM): a exceção pendente não é capturável, nenhum
    /// `catch` a recebe e ela não sai da pendência até o laço de eventos.
    static DESENROLANDO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Grava a exceção pendente e o espelho dela no contexto da thread (que o
/// código gerado lê, `Contexto::pendente`).
fn definir_excecao(v: Option<crate::heap::Valor>) {
    CONTEXTO.with(|c| c.pendente.set(u8::from(v.is_some())));
    EXCEPTION.with(|slot| *slot.borrow_mut() = v);
}

/// Toma a exceção pendente (e limpa o espelho).
fn tomar_excecao() -> Option<crate::heap::Valor> {
    CONTEXTO.with(|c| c.pendente.set(0));
    EXCEPTION.with(|slot| slot.borrow_mut().take())
}

/// Começa a desenrolar o isolado: uma exceção pendente que nenhum `catch`
/// recebe, até o laço de eventos (`isolados.rs`).
fn comecar_desenrolar() {
    DESENROLANDO.with(|d| d.set(true));
    definir_excecao(Some(crate::heap::Valor::Ref(0)));
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
        Some(v) => excecao_como_ref(v),
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
    let value = excecao_da_abi(bits, tag);
    let referencia = ref_da_excecao(value).unwrap_or(0);
    if referencia != 0 {
        // O SDK da fonte reserva o primeiro campo de `Error` para
        // `_stackTrace`; subclasses conservam esse prefixo no layout R7.
        // Os erros internos (ids 1000–1012) mantêm seus índices próprios.
        // O rastro é alocado SEM empréstimo mutável do heap aberto (G6).
        let erro_sdk = CLASS_NAMES.with(|map| {
            map.borrow().iter().find(|(_, nome)| *nome == "Error").map(|(&id, _)| id)
        });
        let precisa = HEAP.with(|heap| match heap.borrow().objeto(referencia) {
            Some(fields) if (1000..=1012).contains(&fields.class_id) && dartforge_is_subclass(fields.class_id, 1007) != 0 => {
                let st_idx = match fields.class_id {
                    1003 => 5,
                    1004 => 7,
                    _ => 1,
                };
                (fields.get(st_idx).map_or(0, |f| f.0) == 0).then_some(st_idx)
            }
            Some(fields) if erro_sdk.is_some_and(|cid| dartforge_is_subclass(fields.class_id, cid) != 0) => {
                fields.first().is_some_and(|(valor, _)| valor == 0).then_some(0)
            }
            _ => None,
        });
        if let Some(st_idx) = precisa {
            // O rastro aloca: o erro fica enraizado até virar a exceção
            // pendente (quem lança nem sempre o enraizou).
            let st = com_raizes(&[referencia], || dartforge_stack_trace_get());
            HEAP.with(|heap| {
                let mut heap = heap.borrow_mut();
                heap.garantir_campos(referencia, st_idx + 1);
                heap.definir_campo(referencia, st_idx, st, true);
            });
        }
    }
    if depurar() {
        let cid = if ref_da_excecao(value).is_some() { dartforge_value_class(referencia) } else { -100 };
        let nome = CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned()).unwrap_or_default();
        eprintln!("[depurar] throw: classe {cid} {nome}");
        mostrar_rastro();
    }
    // G6: a exceção pendente é raiz até ser consumida.
    HEAP.with(|h| h.borrow_mut().set_raiz_do_runtime(0, referencia));
    definir_excecao(Some(value));
}

/// Indica se há exceção pendente na thread corrente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_pending() -> u8 {
    CONTEXTO.with(|c| c.pendente.get())
}

/// Desarma e limpa o slot de exceção pendente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_clear() {
    if desenrolando() {
        // O desenrolar não sai da pendência (`finally`, `return` e saltos
        // também limpam): a pendência chega ao laço de eventos.
        return;
    }
    // Sem exceção nem rastro guardado (o caso de todo `return`): nada a
    // limpar, e o heap não é tocado.
    if CONTEXTO.with(|c| c.pendente.get()) == 0 && CURRENT_STACK_TRACE.with(|slot| slot.borrow().is_none()) {
        return;
    }
    tomar_excecao();
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
    let msg = texto_de_erro(message);
    if ajudante("_dartforgeErroDeFormato").is_some() {
        return com_raizes(&[msg], || dartforge_format_exception_new(msg, 0, -1));
    }
    com_raizes(&[msg], || {
        HEAP.with(|h| {
            // FormatException
            h.borrow_mut().novo_objeto(1001, &[(msg, true), (0, true), (-1, false)])
        })
    })
}

// Construtores de Erros Core

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_exception_new(msg_bits: i64, is_ref: u8) -> i64 {
    HEAP.with(|h| {
        let campos: &[crate::heap::Campo] = if msg_bits == 0 && is_ref == 0 { &[] } else { &[(msg_bits, is_ref != 0)] };
        h.borrow_mut().novo_objeto(1000, campos)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_format_exception_new(msg_handle: i64, src_handle: i64, offset: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroDeFormato") {
        // SAFETY: registrado pelo `dart:core` com a assinatura
        // `(String, Object?, int) -> Object`.
        let g: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
        return com_raizes(&[msg_handle, src_handle], || g(msg_handle, src_handle, offset));
    }
    HEAP.with(|h| {
        h.borrow_mut().novo_objeto(1001, &[(msg_handle, true), (src_handle, true), (offset, false)])
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
    // `indexable_or_len` é uma lista, uma string ou o próprio comprimento
    // (um `int` cru, que só é lido como objeto se for um bloco vivo).
    let len = HEAP.with(|h| {
        let h = h.borrow();
        let x = indexable_or_len;
        if !crate::layout::e_objeto(x) || !h.e_objeto_vivo(x) {
            x
        } else if h.e_lista(x) {
            h.lista_len(x) as i64
        } else {
            h.texto(x).map_or(x, |t| t.len() as i64)
        }
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
        let m = if is_ref != 0 { msg_bits } else { HEAP.with(|h| h.borrow_mut().como_ref(crate::heap::Valor::Int(msg_bits))) };
        return com_raizes(&[m], || g(m));
    }
    alocar_erro_com_rastro(1009, vec![(msg_bits, is_ref != 0)])
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
    let n = HEAP.with(|h| h.borrow().texto(nome).map(|t| t.para_string()).unwrap_or_default());
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
    let msg = texto_de_erro(&texto);
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
    let msg = texto_de_erro("Stack Overflow");
    dartforge_state_error_new(msg)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_no_such_method_error_new(nome: i64) -> i64 {
    if depurar() {
        let t = HEAP.with(|h| h.borrow().texto(nome).map(|t| t.para_string()));
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

/// Campo de um erro do runtime como referência (R5): um escalar guardado
/// (a mensagem de `AssertionError` pode ser um `int`) sai encaixotado; o
/// ausente, null.
fn campo_como_ref(handle: i64, indice: usize) -> i64 {
    let campo = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Some(fields) = heap.objeto(handle) else { return None; };
        fields.get(indice)
    });
    match campo {
        Some((bits, true)) => bits,
        Some((0, false)) | None => 0,
        Some((bits, false)) => HEAP.with(|h| h.borrow_mut().como_ref(crate::heap::Valor::Int(bits))),
    }
}


/// SDK da fonte: o erro construído pela função Dart `nome(texto)`
/// registrada (`identical_patch.dart`), ou `None` sem ela.
fn erro_da_fonte_com_texto(nome: &str, texto: &str) -> Option<i64> {
    let f = ajudante(nome)?;
    // SAFETY: registrado pelo `dart:core` com a assinatura `(String) -> Object`.
    let g: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(f) };
    let t = texto_de_erro(texto);
    Some(com_raizes(&[t], || g(t)))
}
