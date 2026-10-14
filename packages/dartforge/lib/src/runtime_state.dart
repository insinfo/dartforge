import 'dart:isolate';

/// Native identity is observable only for the current isolate's owning thread.
extension DartForgeIsolate on Isolate {
  int? get threadId {
    final id = _isolateThreadIdV1();
    if (id <= 0 || controlPort != Isolate.current.controlPort) return null;
    return id;
  }
}

// Internal bridge used by EventLoop; public exports expose the extension only.
Duration? nextDartEventDeadline() {
  final micros = _eventNextDeadlineMicrosV1();
  return micros < 0 ? null : Duration(microseconds: micros);
}

@pragma('dartforge:runtime-policy-v1')
int _eventNextDeadlineMicrosV1() => -1;

@pragma('dartforge:runtime-policy-v1')
int _isolateThreadIdV1() => 0;
