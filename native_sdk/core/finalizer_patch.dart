// Replaces `_internal/vm/lib/finalizer_patch.dart` (overlay
// `native_sdk/`).
//
// The VM's `Finalizer` lives off its weak GC: `FinalizerEntry.allocate`,
// `exchangeEntriesCollectedWithNull` and the message the GC sends to the
// isolate (`_handleFinalizerMessage`). Here the attachment lives in the
// runtime's collector (`crates/runtime/src/finalizers.rs`): the value and the
// `detach` key are weak, and the action `() => callback(token)` is strong.
// When the collection finds the value dead, the action goes to the queue of
// ready finalizations, which the event loop calls between one event and the
// next (on the VM, a message to the isolate). The callback is bound to the
// creation zone, as on the VM.

part of "core_patch.dart";

@patch
abstract class Finalizer<T> {
  @patch
  factory Finalizer(void Function(T) callback) = _FinalizerImpl<T>;
}

final class _FinalizerImpl<T> implements Finalizer<T> {
  final void Function(T) _callback;

  _FinalizerImpl(void Function(T) callback)
      : _callback = Zone.current.bindUnaryCallbackGuarded(callback);

  void attach(Object value, T token, {Object? detach}) {
    assert(!identical(value, token),
        "The token should not be the value being attached");
    checkValidWeakTarget(value, 'value');
    if (detach != null) {
      checkValidWeakTarget(detach, 'detach');
    }
    _attachFinalizer(
        this, value, _finalizerAction<T>(_callback, token), detach);
  }

  void detach(Object detach) {
    checkValidWeakTarget(detach, 'detach');
    _detachFinalizer(this, detach);
  }
}

/// The action of an attachment: only the callback and the token (not the
/// finalizer nor the value, which must be able to die).
void Function() _finalizerAction<T>(void Function(T) callback, T token) =>
    () => callback(token);

@pragma("vm:external-name", "DartForge_finalizer_attach")
external void _attachFinalizer(
    Object owner, Object val, void Function() action, Object? detachToken);

@pragma("vm:external-name", "DartForge_finalizer_detach")
external void _detachFinalizer(Object owner, Object detachToken);
