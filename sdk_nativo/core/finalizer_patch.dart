// Substitui `_internal/vm/lib/finalizer_patch.dart` (sobreposição
// `sdk_nativo/`).
//
// O `Finalizer` da VM vive do GC fraco dela: `FinalizerEntry.allocate`,
// `exchangeEntriesCollectedWithNull` e a mensagem que o GC manda ao isolado
// (`_handleFinalizerMessage`). Aqui o anexo mora no coletor do runtime
// (`crates/runtime/src/finalizadores.rs`): valor e chave de `detach` fracos,
// e a ação `() => callback(token)` forte. Quando a coleta acha o valor
// morto, a ação vai para a fila de finalizações prontas, que o laço de
// eventos chama entre um evento e outro (na VM, uma mensagem ao isolado).
// O callback é ligado à zona da criação, como na VM.

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
    _anexarFinalizador(
        this, value, _acaoDeFinalizador<T>(_callback, token), detach);
  }

  void detach(Object detach) {
    checkValidWeakTarget(detach, 'detach');
    _desanexarFinalizador(this, detach);
  }
}

/// A ação de um anexo: só o callback e o token (não o finalizador nem o
/// valor, que precisa poder morrer).
void Function() _acaoDeFinalizador<T>(void Function(T) callback, T token) =>
    () => callback(token);

@pragma("vm:external-name", "DartForge_finalizador_anexar")
external void _anexarFinalizador(
    Object dono, Object valor, void Function() acao, Object? desanexo);

@pragma("vm:external-name", "DartForge_finalizador_desanexar")
external void _desanexarFinalizador(Object dono, Object desanexo);
