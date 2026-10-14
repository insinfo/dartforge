import 'dart:async';
import 'sync_native_callable.dart';
import 'runtime_state.dart';

/// Opt-in OS waiting on the main isolate, with portable unsupported fallbacks.
///
/// This currently integrates macOS CFRunLoop sources and the main dispatch
/// queue. The application remains responsible for AppKit NSEvent dispatch.
/// It does not service nested Dart turns inside a native callback.
abstract final class EventLoop {
  /// Snapshot of time until ready Dart work or the next active timer.
  /// Zero means work is ready or a microtask drain is in progress; null means
  /// none or the owner is paused/unsupported.
  /// This query does not transfer scheduling ownership or enable nested turns.
  static Duration? get nextDeadline => nextDartEventDeadline();

  /// Whether this platform and isolate support the native wait adapter.
  static bool get supportsNativePump => _supportsNativePumpV1();

  /// Whether this owner supports an application dispatcher on outer Dart turns.
  static bool get supportsNativeEventDispatcher =>
      _supportsNativeEventDispatcherV1();

  /// Registers one application callback for bounded native event work.
  ///
  /// The native pump must already be enabled. The callback runs after native
  /// waiting returns, outside foreign callback frames, after queued microtasks
  /// and control requests. It may replace/clear itself or disable the pump.
  /// This does not provide nested Dart turns or AppKit modal-loop progress.
  /// Replacement releases the previous registration; disable clears it.
  /// Throws UnsupportedError when unavailable, or StateError for an invalid
  /// owner or a disabled pump. The callback must bound its own work and may
  /// schedule a microtask when more native events remain.
  static void setNativeEventDispatcher(void Function() callback) =>
      _checkStatus(_setNativeEventDispatcherV1(callback));

  /// Clears the application callback while the pump is enabled.
  ///
  /// Repeated clears are idempotent. Throws UnsupportedError when unavailable,
  /// or StateError for an invalid owner or a disabled pump.
  static void clearNativeEventDispatcher() =>
      _checkStatus(_clearNativeEventDispatcherV1());

  /// Nested Dart event turns are not part of this contract.
  static bool get supportsNestedTurns => false;

  /// Whether the separate owner-thread synchronous callback API is available.
  /// This does not enable nested turns or certify realtime audio execution.
  static bool get supportsSyncCallbacks => SyncNativeCallable.isSupported;

  /// Enables native waiting and keeps the event loop alive until disabled.
  ///
  /// Repeated calls are idempotent; this is application lifecycle state, not
  /// a reference-counted scope. On unsupported runtimes throws UnsupportedError.
  /// Throws StateError when invoked outside the owner thread or OS setup fails.
  ///
  /// ```dart
  /// if (EventLoop.supportsNativePump) {
  ///   EventLoop.enableNativePump();
  ///   // Close the application with disableNativePump().
  /// }
  /// ```
  static void enableNativePump() => _checkStatus(_enableNativePumpV1());

  /// Stops native waiting and releases the application keep-alive.
  ///
  /// Idempotent on supported owners. Throws UnsupportedError on unsupported
  /// platforms, or StateError for an invalid owner or OS failure.
  static void disableNativePump() => _checkStatus(_disableNativePumpV1());

  /// Requests shutdown after the current callback returns to the Dart loop.
  ///
  /// Does not drain events inside a foreign frame. Throws UnsupportedError
  /// when native waiting is unavailable in this isolate.
  static void requestNativePumpShutdown() {
    if (!supportsNativePump)
      throw UnsupportedError('Native waiting is unavailable in this isolate');
    scheduleMicrotask(disableNativePump);
  }

  static void _checkStatus(int status) {
    if (status == 1) return;
    if (status == 0)
      throw UnsupportedError('Native waiting is unavailable on this runtime');
    if (status == -1)
      throw StateError('Native waiting requires the process main thread');
    if (status == -3)
      throw StateError(
          'The native pump must be enabled before registering event work');
    throw StateError('Native waiting could not initialize OS resources');
  }
}

// Versioned declaration and library identity are checked by native lowering.
// Bodies are valid Dart fallbacks; annotations alone confer no capability.
@pragma('dartforge:native-pump-v1')
bool _supportsNativePumpV1() => false;

@pragma('dartforge:native-pump-v1')
int _enableNativePumpV1() => 0;

@pragma('dartforge:native-pump-v1')
int _disableNativePumpV1() => 0;

@pragma('dartforge:native-pump-v1')
bool _supportsNativeEventDispatcherV1() => false;

@pragma('dartforge:native-pump-v1')
int _setNativeEventDispatcherV1(void Function() callback) => 0;

@pragma('dartforge:native-pump-v1')
int _clearNativeEventDispatcherV1() => 0;
