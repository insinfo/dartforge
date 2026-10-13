// Replaces `_internal/vm/lib/schedule_microtask_patch.dart` (overlay
// `native_sdk/`). The microtask queue and its order are those of the source's
// `schedule_microtask.dart` (`_nextCallback`, `_startMicrotaskLoop`); the
// runtime only needs to run the closure `dart:async` asks for, before any
// timer (the event loop of docs/NATIVO-PLANO.md, P6). On the VM, the one that
// delivers this closure is `dart:isolate` (`_setScheduleImmediateClosure`),
// tied to the isolate's message port; here it is a native.

part of "async_patch.dart";

@patch
class _AsyncRun {
  @patch
  static void _scheduleImmediate(void callback()) {
    _scheduleImmediateNative(callback);
  }

  /// Queues `callback` for the event loop to run before the next timer. The
  /// runtime keeps the closure as a root until it runs it.
  @pragma("vm:external-name", "DartForge_scheduleImmediate")
  external static void _scheduleImmediateNative(void Function() callback);
}
