// Substitui `_internal/vm/lib/finalizer_patch.dart` (sobreposição
// `sdk_nativo/`).
//
// O `Finalizer` da VM vive do GC fraco dela: `FinalizerEntry.allocate`,
// `exchangeEntriesCollectedWithNull` e a mensagem que o GC manda ao isolado
// (`_handleFinalizerMessage`) — natives e intrínsecos que dependem de
// referências fracas no coletor. O coletor do nativo ainda não tem
// referência fraca (vem com o experimento ARC, PLANO.md). Até lá, este
// finalizador valida os argumentos como a VM e nunca roda o callback — o que
// a especificação permite: "there is no promise that a finalizer will ever
// be run" (`Finalizer`, dart:core/weak.dart). Nenhum programa pode observar
// a diferença pela semântica garantida.

part of "core_patch.dart";

@patch
abstract class Finalizer<T> {
  @patch
  factory Finalizer(void Function(T) callback) = _FinalizerImpl<T>;
}

final class _FinalizerImpl<T> implements Finalizer<T> {
  // O callback fica registrado na zona, como na VM, para o dia em que o
  // coletor rodar finalizadores.
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
  }

  void detach(Object detach) {
    checkValidWeakTarget(detach, 'detach');
  }
}
