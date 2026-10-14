import 'dart:ffi';

import 'scoped_resource.dart';

/// A synchronous native callback delivered on its owning isolate's thread.
///
/// Foreign callers block until the owner finishes the callback. This is not a
/// realtime audio entry: delivery uses a queue, allocation and synchronization.
/// The owner must return to its event loop to accept a foreign invocation.
final class SyncNativeCallable<T extends Function> implements ScopedResource {
  final NativeCallable<T> _delegate;

  SyncNativeCallable._(this._delegate);

  /// Creates a callable for scalar, pointer or void native return signatures.
  ///
  /// [queueTimeout] limits waiting before the owner starts the invocation. An
  /// expired request returns [exceptionalReturn] without executing the callback.
  /// Once started, the caller waits until completion so native argument storage
  /// remains valid. The timeout cannot interrupt a running Dart callback.
  /// The callback must be synchronous, match the native signature's Dart
  /// argument/return types, and finish using borrowed native buffers before
  /// returning. An async void function does not satisfy this precondition.
  /// Calls from the owner thread execute directly. Closing cancels pending
  /// requests and preserves an invocation already running on the owner.
  /// Handle arguments/returns and aggregate returns are currently unsupported.
  /// The official Dart VM throws UnsupportedError without creating a callback.
  ///
  /// ```dart
  /// if (SyncNativeCallable.isSupported) {
  ///   final callback = SyncNativeCallable<Int32 Function(Int32)>(
  ///     (int value) => value + 1,
  ///     exceptionalReturn: -1,
  ///   );
  ///   // Register callback.nativeFunction with the native producer.
  ///   // Stop the producer and finish its calls before disposing resources.
  ///   callback.close();
  /// }
  /// ```
  factory SyncNativeCallable(
    Function callback, {
    Object? exceptionalReturn,
    Duration queueTimeout = const Duration(seconds: 1),
  }) {
    if (queueTimeout.inMicroseconds <= 0) {
      throw ArgumentError.value(
          queueTimeout, 'queueTimeout', 'Must be positive');
    }
    return SyncNativeCallable._(_createSyncNativeCallableV1<T>(
      callback,
      exceptionalReturn,
      queueTimeout.inMicroseconds,
    ));
  }

  /// Whether this runtime implements owner-thread synchronous delivery.
  static bool get isSupported => _supportsSyncNativeCallableV1();

  /// The native function pointer, valid while its native producer is registered.
  Pointer<NativeFunction<T>> get nativeFunction => _delegate.nativeFunction;

  /// Whether registration keeps the owning isolate alive.
  bool get keepIsolateAlive => _delegate.keepIsolateAlive;

  set keepIsolateAlive(bool value) => _delegate.keepIsolateAlive = value;

  /// Retires this callable and cancels invocations that have not started.
  void close() => _delegate.close();

  @override
  void dispose() => close();
}

@pragma('dartforge:sync-callback-v1')
bool _supportsSyncNativeCallableV1() => false;

@pragma('dartforge:sync-callback-v1')
NativeCallable<T> _createSyncNativeCallableV1<T extends Function>(
  Function callback,
  Object? exceptionalReturn,
  int queueTimeoutMicros,
) =>
    throw UnsupportedError(
        'Synchronous owner-thread callbacks are unavailable');
