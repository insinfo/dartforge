// Replaces `_internal/vm/lib/ffi_native_finalizer_patch.dart` (overlay
// `native_sdk/`).
//
// The VM's `NativeFinalizer` lives off its GC's `FinalizerEntry`. Here the
// attachment lives in the runtime's collector
// (`crates/runtime/src/finalizers.rs`): the value and the `detach` key are
// weak, the (C callback, token) pair kept in the attachment. When the
// collection finds the value dead, it calls `callback(token)` in the
// collection itself, like the VM; at the end of the isolate it runs the remaining ones.

// All imports must be in all FFI patch files to not depend on the order
// the patches are applied.
import 'dart:_internal';
import 'dart:isolate';
import 'dart:typed_data';

@patch
@pragma("vm:entry-point")
abstract interface class Finalizable {}

@patch
@pragma("vm:entry-point")
abstract final class NativeFinalizer {
  @patch
  factory NativeFinalizer(Pointer<NativeFinalizerFunction> callback) =
      _NativeFinalizer;
}

final class _NativeFinalizer implements NativeFinalizer {
  final Pointer<NativeFinalizerFunction> _callback;

  _NativeFinalizer(this._callback);

  void attach(
    Finalizable value,
    Pointer<Void> token, {
    Object? detach,
    int? externalSize,
  }) {
    externalSize ??= 0;
    RangeError.checkNotNegative(externalSize, 'externalSize');
    if (detach != null) {
      checkValidWeakTarget(detach, 'detach');
    }
    _attachNativeFinalizer(
        this, value, _callback.address, token.address, detach, externalSize);
  }

  void detach(Object detach) {
    checkValidWeakTarget(detach, 'detach');
    _detachNativeFinalizer(this, detach);
  }
}

/// The owner of the finalizers of `asTypedList(finalizer:)`.
final _NativeFinalizer _asTypedListFinalizer = _NativeFinalizer(nullptr.cast());

@patch
void _attachAsTypedListFinalizer(
  Pointer<NativeFinalizerFunction> callback,
  Object typedList,
  Pointer pointer,
  int? externalSize,
) {
  _attachNativeFinalizer(_asTypedListFinalizer, typedList, callback.address,
      pointer.address, null, externalSize ?? 0);
}

@pragma("vm:external-name", "DartForge_finalizer_attach_native")
external void _attachNativeFinalizer(
    Object owner, Object val, int function, int token, Object? detachToken, int extSize);

@pragma("vm:external-name", "DartForge_finalizer_detach")
external void _detachNativeFinalizer(Object owner, Object detachToken);
