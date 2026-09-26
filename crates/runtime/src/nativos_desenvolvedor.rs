// Runtime nativo: os natives de `dart:developer` (`runtime/lib/developer.cc`
// e `timeline.cc` da VM) no perfil de um executável AOT — o `PRODUCT` da
// VM, sem serviço de depuração nem timeline: extensões de serviço não são
// registradas, eventos e logs não vão a lugar algum, e quem pede
// informações do servidor recebe `null` na porta.

/// O id de uma `SendPort` (o `_SendPort._id` da sobreposição de
/// `dart:isolate`: o primeiro campo).
fn id_da_send_port(porta: i64) -> i64 {
    campo_nativo(porta)
}

/// `Developer_debugger(when, message)`: sem depurador, devolve `when`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_debugger(quando: u8, _mensagem: i64) -> u8 {
    quando
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_inspect(objeto: i64) -> i64 {
    objeto
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn dartforge_nativo_Developer_log(
    _mensagem: i64,
    _instante: i64,
    _sequencia: i64,
    _nivel: i64,
    _nome: i64,
    _zona: i64,
    _erro: i64,
    _rastro: i64,
) -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_postEvent(_tipo: i64, _dados: i64) {}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_lookupExtension(_metodo: i64) -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_registerExtension(_metodo: i64, _tratador: i64) -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_getServiceMajorVersion() -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_getServiceMinorVersion() -> i64 {
    0
}

/// `Developer_getServerInfo(port)`: sem servidor, `null` na porta.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_getServerInfo(porta: i64) {
    postar(id_da_send_port(porta), Grafo::escalar(0, ValueTag::Ref));
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_webServerControl(porta: i64, _ligar: u8, _silenciar: i64) {
    postar(id_da_send_port(porta), Grafo::escalar(0, ValueTag::Ref));
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_getIsolateIdFromSendPort(_porta: i64) -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_getObjectId(_objeto: i64) -> i64 {
    0
}

/// `Developer_reachability_barrier`: quantas coletas completas já houve (a
/// barreira da VM avança a cada uma).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_reachability_barrier() -> i64 {
    HEAP.with(|h| h.borrow().stats().collections as i64)
}

/// `Developer_NativeRuntime_buildId`: o executável não tem o id de build
/// de um snapshot AOT da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_NativeRuntime_buildId() -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Timeline_isDartStreamEnabled() -> u8 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Timeline_getNextTaskId() -> i64 {
    0
}

/// `Timeline_getTraceClock`: o relógio monotônico em microssegundos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Timeline_getTraceClock() -> i64 {
    dartforge_nativo_Stopwatch_now() / 1000
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Timeline_reportTaskEvent(_id: i64, _fluxo: i64, _tipo: i64, _nome: i64, _args: i64) {}

/// `SendPort.nativePort` (o `NativePort` do `dart:ffi`): o id da porta.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SendPort_get_id(porta: i64) -> i64 {
    id_da_send_port(porta)
}
