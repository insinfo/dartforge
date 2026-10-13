// Runtime nativo: os natives de `dart:developer` (`runtime/lib/developer.cc`
// e `timeline.cc` da VM) no perfil de um executável AOT — o `PRODUCT` da
// VM, sem serviço de depuração nem timeline: extensões de serviço não são
// registradas, eventos e logs não vão a lugar algum, e quem pede
// informações do servidor recebe `null` na porta.

/// O id de uma `SendPort` (o `_SendPort._id` da sobreposição de
/// `dart:isolate`: o primeiro campo).
fn send_port_id(port: i64) -> i64 {
    native_field(port)
}

/// `Developer_debugger(when, message)`: sem depurador, devolve `when`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_debugger(when: u8, _message: i64) -> u8 {
    when
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_inspect(object: i64) -> i64 {
    object
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn dartforge_nativo_Developer_log(
    _message: i64,
    _instant: i64,
    _sequence: i64,
    _level: i64,
    _name: i64,
    _zone: i64,
    _error: i64,
    _trace: i64,
) -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_postEvent(_type: i64, _dados: i64) {}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_lookupExtension(_method: i64) -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_registerExtension(_method: i64, _handler: i64) -> i64 {
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
pub extern "C" fn dartforge_nativo_Developer_getServerInfo(port: i64) {
    post(send_port_id(port), Graph::null_val());
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_webServerControl(port: i64, _enable: u8, _silence: i64) {
    post(send_port_id(port), Graph::null_val());
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_getIsolateIdFromSendPort(_port: i64) -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Developer_getObjectId(_object: i64) -> i64 {
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
pub extern "C" fn dartforge_nativo_Timeline_reportTaskEvent(_id: i64, _stream: i64, _type: i64, _name: i64, _args: i64) {}

/// `SendPort.nativePort` (o `NativePort` do `dart:ffi`): o id da porta.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_SendPort_get_id(port: i64) -> i64 {
    send_port_id(port)
}
