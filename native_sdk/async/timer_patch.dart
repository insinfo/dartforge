// Replaces `_internal/vm/lib/timer_patch.dart` (overlay `native_sdk/`).
//
// On the VM, `Timer` is created by `VMLibraryHooks.timerFactory`, which
// `dart:isolate` (`timer_impl.dart`) binds to a timer heap woken by
// `RawReceivePort` and by `dart:io`'s *event handler*. Here the timer heap
// belongs to the runtime (the event loop of docs/NATIVO-PLANO.md §4.3, P6):
// `(deadline, seq)` with a monotonic clock, `seq` breaking ties between equal
// deadlines in creation order — the determinism the corpus requires. The
// runtime calls `_Timer._fire` when the deadline expires, always after
// draining the microtasks.

part of "async_patch.dart";

@patch
class Timer {
  @patch
  static Timer _createTimer(Duration duration, void callback()) {
    int milliseconds = duration.inMilliseconds;
    if (milliseconds < 0) milliseconds = 0;
    return _Timer._(milliseconds, (_) {
      callback();
    }, false);
  }

  @patch
  static Timer _createPeriodicTimer(
      Duration duration, void callback(Timer timer)) {
    int milliseconds = duration.inMilliseconds;
    if (milliseconds < 0) milliseconds = 0;
    return _Timer._(milliseconds, callback, true);
  }
}

/// A runtime timer: the id is that of the event loop's timer heap
/// (0 = inactive).
final class _Timer implements Timer {
  final void Function(Timer) _callback;
  final bool _periodic;
  int _id = 0;
  int _tick = 0;

  _Timer._(int milliseconds, this._callback, this._periodic) {
    _id = _create(milliseconds, _fire, _periodic);
  }

  bool get isActive => _id != 0;

  int get tick => _tick;

  void cancel() {
    final id = _id;
    if (id == 0) return;
    _id = 0;
    _cancelNative(id);
  }

  /// Called by the event loop when the deadline expires. A periodic timer stays
  /// scheduled (the runtime reinserts it with the next deadline) until
  /// `cancel`.
  @pragma("vm:entry-point", "call")
  void _fire() {
    if (_id == 0) return;
    _tick++;
    if (!_periodic) _id = 0;
    _callback(this);
  }

  /// Schedules the timer; returns the id (never 0). The runtime keeps `fire`
  /// (the tear-off of `_fire`, with this `_Timer` in the environment) as a root
  /// while it is scheduled and calls it at the deadline — the event loop only
  /// calls closures without arguments, the only point where the runtime calls Dart.
  @pragma("vm:external-name", "DartForge_Timer_new")
  external static int _create(
      int milliseconds, void Function() fire, bool periodic);

  /// Removes the timer from the timer heap (the root is released).
  @pragma("vm:external-name", "DartForge_Timer_cancel")
  external static void _cancelNative(int id);
}
