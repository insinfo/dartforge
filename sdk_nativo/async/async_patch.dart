// Patch do `dart:async` para o backend nativo do DartForge (sobreposição
// `sdk_nativo/`, docs/NATIVO-PLANO.md §7). Substitui
// `_internal/vm/lib/async_patch.dart` da seção `vm` do SDK 3.6.2.
//
// Por que não o da VM: o `async`/`sync*`/`async*` da VM suspende o quadro de
// máquina (`_SuspendState._resume`/`_clone`, intrínsecos sem fallback), o que
// é incompatível com a pilha-sombra explícita do nativo. O modelo daqui é o do
// dart2js (`_internal/js_runtime/lib/async_patch.dart`): o corpo `async` vira
// uma máquina de estados (P6), o quadro mora no heap, e cada `await` registra
// a continuação `corpo(códigoDeStatus, resultado)`. Os membros abaixo são os
// do dart2js, sem `JS(...)`: o laço de exceção que o dart2js escreve em JS
// (`_wrapJsFunctionForAsync`) está escrito em Dart em `_envolverCorpo`.
//
// O que o `dart:async` da fonte exige de um patch é só isto (as quatro
// `external` dele): `_trySetStackTrace`, `Timer._createTimer`,
// `Timer._createPeriodicTimer` (`timer_patch.dart`) e
// `_AsyncRun._scheduleImmediate` (`schedule_microtask_patch.dart`). O resto é
// o apoio que o lowering de `async` chama.

import "dart:_internal" show patch, unsafeCast;

part "schedule_microtask_patch.dart";
part "timer_patch.dart";

@patch
@pragma("vm:external-name", "Error_trySetStackTrace")
external void _trySetStackTrace(Object error, StackTrace stackTrace);

/// Códigos de status da continuação de um corpo `async` (os de
/// `dart:_async_status_codes` do dart2js).
const int _SUCESSO = 0;
const int _ERRO = 1;

/// A continuação de um corpo `async` transformado em máquina de estados:
/// `corpo(_SUCESSO, valor)` retoma com o valor do `await`; `corpo(_ERRO,
/// erro)` retoma lançando `erro` no ponto do `await` (§4.2 do plano).
typedef _WrappedAsyncBody = void Function(int errorCode, dynamic result);

/// O par (erro, rastro) que a continuação recebe com `_ERRO`.
final class _ErroAssincrono {
  final Object erro;
  final StackTrace rastro;
  _ErroAssincrono(this.erro, this.rastro);
}

class _AsyncAwaitCompleter<T> implements Completer<T> {
  final _future = _Future<T>();
  bool isSync;

  _AsyncAwaitCompleter() : isSync = false;

  void complete([FutureOr<T>? value]) {
    // Todo caminho exige que, se o valor é null, `null as T` passe.
    value = (value == null) ? value as T : value;
    if (!isSync) {
      _future._asyncComplete(value);
    } else if (value is Future<T>) {
      assert(!_future._isComplete);
      _future._chainFuture(value);
    } else {
      _future._completeWithValue(value);
    }
  }

  void completeError(Object e, [StackTrace? st]) {
    st ??= AsyncError.defaultStackTrace(e);
    if (isSync) {
      _future._completeError(e, st);
    } else {
      _future._asyncCompleteError(e, st);
    }
  }

  Future<T> get future => _future;
  bool get isCompleted => !_future._mayComplete;
}

/// Cria o `Completer` de uma função `async`.
@pragma("vm:entry-point", "call")
Completer<T> _makeAsyncAwaitCompleter<T>() {
  return _AsyncAwaitCompleter<T>();
}

/// Começa o corpo de uma função `async` de forma síncrona e devolve o
/// `Future` dela.
@pragma("vm:entry-point", "call")
dynamic _asyncStartSync(
    _WrappedAsyncBody bodyFunction, _AsyncAwaitCompleter completer) {
  bodyFunction(_SUCESSO, null);
  completer.isSync = true;
  return completer.future;
}

/// O `await` de uma função `async`: registra a continuação no objeto.
@pragma("vm:entry-point", "call")
dynamic _asyncAwait(dynamic object, _WrappedAsyncBody bodyFunction) {
  _awaitOnObject(object, bodyFunction);
}

/// O `return` de uma função `async`.
@pragma("vm:entry-point", "call")
dynamic _asyncReturn(dynamic object, Completer completer) {
  completer.complete(object);
}

/// A exceção não capturada de uma função `async` (o modelo por valor: o
/// corpo termina com a exceção pendente, que chega aqui como valor).
@pragma("vm:entry-point", "call")
dynamic _asyncRethrow(Object error, StackTrace stackTrace, Completer completer) {
  completer.completeError(error, stackTrace);
}

/// Espera `object`: se é `Future`, registra a continuação nele; senão, num
/// `_Future` já completado com ele (a ordem de microtarefas é a da fonte).
void _awaitOnObject(object, _WrappedAsyncBody bodyFunction) {
  FutureOr<dynamic> Function(dynamic) thenCallback =
      (result) => bodyFunction(_SUCESSO, result);

  Function errorCallback = (Object error, StackTrace stackTrace) {
    bodyFunction(_ERRO, _ErroAssincrono(error, stackTrace));
  };

  if (object is _Future) {
    // A continuação já foi registrada na zona (`_envolverCorpo`).
    object._thenAwait(thenCallback, errorCallback);
  } else if (object is Future) {
    object.then(thenCallback, onError: errorCallback);
  } else {
    _Future future = _Future().._setValue(object);
    future._thenAwait(thenCallback, errorCallback);
  }
}

/// Registra o corpo na zona corrente e o protege: se a retomada lança, o
/// corpo é chamado de novo com `_ERRO` — o laço que o dart2js escreve em JS
/// em `_wrapJsFunctionForAsync`.
@pragma("vm:entry-point", "call")
_WrappedAsyncBody _envolverCorpo(_WrappedAsyncBody corpo) {
  void protegido(int codigo, dynamic resultado) {
    while (true) {
      try {
        corpo(codigo, resultado);
        break;
      } catch (e, s) {
        resultado = _ErroAssincrono(e, s);
        codigo = _ERRO;
      }
    }
  }

  return Zone.current.registerBinaryCallback<void, int, dynamic>(protegido);
}
