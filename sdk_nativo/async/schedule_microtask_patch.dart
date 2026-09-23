// Substitui `_internal/vm/lib/schedule_microtask_patch.dart` (sobreposição
// `sdk_nativo/`). A fila de microtarefas e a ordem delas são as do
// `schedule_microtask.dart` da fonte (`_nextCallback`,
// `_startMicrotaskLoop`); o runtime só precisa rodar a closure que o
// `dart:async` pede, antes de qualquer timer (o laço de eventos de
// docs/NATIVO-PLANO.md, P6). Na VM, quem entrega essa closure é o
// `dart:isolate` (`_setScheduleImmediateClosure`), ligado à porta de
// mensagens do isolado; aqui é um native.

part of "async_patch.dart";

@patch
class _AsyncRun {
  @patch
  static void _scheduleImmediate(void callback()) {
    _agendarImediato(callback);
  }

  /// Enfileira `callback` para o laço de eventos rodar antes do próximo
  /// timer. O runtime guarda a closure como raiz até rodá-la.
  @pragma("vm:external-name", "DartForge_scheduleImmediate")
  external static void _agendarImediato(void Function() callback);
}
