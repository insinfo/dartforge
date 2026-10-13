// Native runtime: the `dart:developer` natives (the VM's `runtime/lib/developer.cc`
// and `timeline.cc`) in the profile of an AOT executable — the VM's `PRODUCT`,
// with no debugging service and no timeline: service extensions are not
// registered, events and logs go nowhere, and whoever asks for server
// information receives `null` on the port.

/// The id of a `SendPort` (the `_SendPort._id` of the `dart:isolate` overlay:
/// the first field).
fn send_port_id(port: i64) -> i64 {
    native_field(port)
}

/// `Developer_debugger(when, message)`: without a debugger, returns `when`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_debugger(when: u8, _message: i64) -> u8 {
    when
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_inspect(object: i64) -> i64 {
    object
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn dartforge_native_Developer_log(
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
pub extern "C" fn dartforge_native_Developer_postEvent(_type: i64, _data: i64) {}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_lookupExtension(_method: i64) -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_registerExtension(_method: i64, _handler: i64) -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_getServiceMajorVersion() -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_getServiceMinorVersion() -> i64 {
    0
}

/// `Developer_getServerInfo(port)`: without a server, `null` on the port.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_getServerInfo(port: i64) {
    post(send_port_id(port), Graph::null_val());
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_webServerControl(port: i64, _enable: u8, _silence: i64) {
    post(send_port_id(port), Graph::null_val());
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_getIsolateIdFromSendPort(_port: i64) -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_getObjectId(_object: i64) -> i64 {
    0
}

/// `Developer_reachability_barrier`: how many full collections have happened (the
/// VM's barrier advances on each one).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_reachability_barrier() -> i64 {
    HEAP.with(|h| h.borrow().stats().collections as i64)
}

/// `Developer_NativeRuntime_buildId`: the executable does not have the build id
/// of a VM AOT snapshot.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Developer_NativeRuntime_buildId() -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Timeline_isDartStreamEnabled() -> u8 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Timeline_getNextTaskId() -> i64 {
    0
}

/// `Timeline_getTraceClock`: the monotonic clock in microseconds.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Timeline_getTraceClock() -> i64 {
    dartforge_native_Stopwatch_now() / 1000
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Timeline_reportTaskEvent(_id: i64, _stream: i64, _type: i64, _name: i64, _args: i64) {}

/// `SendPort.nativePort` (the `NativePort` of `dart:ffi`): the port id.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_SendPort_get_id(port: i64) -> i64 {
    send_port_id(port)
}
