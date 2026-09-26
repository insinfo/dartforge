// Substitui `_internal/vm/lib/ffi_native_finalizer_patch.dart` (sobreposição
// `sdk_nativo/`).
//
// O `NativeFinalizer` da VM vive do `FinalizerEntry` do GC dela. Aqui o anexo
// mora no coletor do runtime (`crates/runtime/src/finalizadores.rs`): o
// valor e a chave de `detach` fracos, o par (callback C, token) guardado no
// anexo. Quando a coleta acha o valor morto, chama `callback(token)` na
// própria coleta, como a VM; no fim do isolado roda os que restarem.

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
    _anexarFinalizadorNativo(
        this, value, _callback.address, token.address, detach);
  }

  void detach(Object detach) {
    checkValidWeakTarget(detach, 'detach');
    _desanexarFinalizadorNativo(this, detach);
  }
}

/// O dono dos finalizadores de `asTypedList(finalizer:)`.
final _NativeFinalizer _asTypedListFinalizer = _NativeFinalizer(nullptr.cast());

@patch
void _attachAsTypedListFinalizer(
  Pointer<NativeFinalizerFunction> callback,
  Object typedList,
  Pointer pointer,
  int? externalSize,
) {
  _anexarFinalizadorNativo(_asTypedListFinalizer, typedList, callback.address,
      pointer.address, null);
}

@pragma("vm:external-name", "DartForge_finalizador_anexar_nativo")
external void _anexarFinalizadorNativo(
    Object dono, Object valor, int funcao, int token, Object? desanexo);

@pragma("vm:external-name", "DartForge_finalizador_desanexar")
external void _desanexarFinalizadorNativo(Object dono, Object desanexo);
