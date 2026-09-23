// Substitui `_internal/vm/lib/timer_patch.dart` (sobreposição `sdk_nativo/`).
//
// Na VM, `Timer` é criado por `VMLibraryHooks.timerFactory`, que o
// `dart:isolate` (`timer_impl.dart`) liga a um heap de timers acordado por
// `RawReceivePort` e pelo *event handler* do `dart:io`. Aqui o heap de
// timers é do runtime (o laço de eventos de docs/NATIVO-PLANO.md §4.3, P6):
// `(prazo, seq)` com relógio monotônico, `seq` desempatando prazos iguais na
// ordem de criação — o determinismo que o corpus exige. O runtime chama
// `_Timer._disparar` quando o prazo vence, sempre depois de esvaziar as
// microtarefas.

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

/// Um timer do runtime: o id é o do heap de timers do laço de eventos
/// (0 = inativo).
final class _Timer implements Timer {
  final void Function(Timer) _callback;
  final bool _periodico;
  int _id = 0;
  int _tick = 0;

  _Timer._(int milliseconds, this._callback, this._periodico) {
    _id = _novo(milliseconds, this, _periodico);
  }

  bool get isActive => _id != 0;

  int get tick => _tick;

  void cancel() {
    final id = _id;
    if (id == 0) return;
    _id = 0;
    _cancelar(id);
  }

  /// Chamado pelo laço de eventos quando o prazo vence. Um timer periódico
  /// continua agendado (o runtime o reinsere com o próximo prazo) até
  /// `cancel`.
  @pragma("vm:entry-point", "call")
  void _disparar() {
    if (_id == 0) return;
    _tick++;
    if (!_periodico) _id = 0;
    _callback(this);
  }

  /// Agenda o timer; devolve o id (nunca 0). O runtime guarda o `_Timer`
  /// como raiz enquanto ele estiver agendado.
  @pragma("vm:external-name", "DartForge_Timer_novo")
  external static int _novo(int milliseconds, _Timer timer, bool periodico);

  /// Tira o timer do heap de timers (a raiz é solta).
  @pragma("vm:external-name", "DartForge_Timer_cancelar")
  external static void _cancelar(int id);
}
