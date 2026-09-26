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

// ─── sync* ──────────────────────────────────────────────────────────────────
//
// O modelo é o da VM (`_internal/vm/lib/async_patch.dart`, `_SyncStarIterable`
// e `_SyncStarIterator`), com a máquina de estados no lugar do
// `_SuspendState`: chamar a função `sync*` não roda nada e devolve o
// iterável; cada `iterator` começa o corpo do início, com um quadro novo
// (`_novoCorpo`, o equivalente do `_stateAtStart._clone()` da VM).
//
// O corpo é `corpo(iterador, código, valor)` e devolve se há mais elementos:
// `yield e` grava `_current` e devolve `true`; `yield* e` grava
// `_yieldStarIterable` e devolve `true`; o fim do corpo devolve `false`. Uma
// exceção sai do corpo como de qualquer função, e `moveNext` a trata como a
// VM; `código == _ERRO` retoma lançando o `_ErroAssincrono` no ponto do
// `yield*` (a exceção do iterador aninhado).

/// O corpo de uma função `sync*` transformado em máquina de estados:
/// `bool corpo(_SyncStarIterator iterador, int codigo, Object? valor)`. Fica
/// `Function` (chamado dinamicamente) porque a closure que o lowering cria não
/// carrega o tipo de função reificado, e um teste contra o tipo exato falharia.
typedef _CorpoSyncStar = Function;

/// Cria o iterável de uma chamada de função `sync*`.
@pragma("vm:entry-point", "call")
_SyncStarIterable<T> _makeSyncStarIterable<T>(Function novo) {
  return _SyncStarIterable<T>(novo);
}

/// `yield e` num corpo `sync*`.
@pragma("vm:entry-point", "call")
bool _syncStarYield(_SyncStarIterator iterador, Object? valor) {
  iterador._current = valor;
  return true;
}

/// `yield* e` num corpo `sync*`.
@pragma("vm:entry-point", "call")
bool _syncStarYieldStar(_SyncStarIterator iterador, Iterable iteravel) {
  iterador._yieldStarIterable = iteravel;
  return true;
}

class _SyncStarIterable<T> extends Iterable<T> {
  // `_CorpoSyncStar Function()`, sem o tipo exato (ver `_CorpoSyncStar`).
  final Function _novoCorpo;

  _SyncStarIterable(this._novoCorpo);

  Iterator<T> get iterator {
    return _SyncStarIterator<T>(_novoCorpo());
  }
}

class _SyncStarIterator<T> implements Iterator<T> {
  _CorpoSyncStar? _state;
  Iterator<T>? _yieldStarIterator;

  // Pilha dos corpos sync* suspensos num `yield*` de outro sync*.
  List<_CorpoSyncStar>? _stack;

  // O corpo grava `_current` ou `_yieldStarIterable` antes de suspender.
  T? _current;
  Iterable<T>? _yieldStarIterable;

  @override
  T get current => _current as T;

  _SyncStarIterator(_CorpoSyncStar state) : _state = state;

  bool _handleSyncStarMethodCompletion() {
    _current = null;
    _state = null;
    final stack = _stack;
    if (stack != null && stack.isNotEmpty) {
      _state = stack.removeLast();
      return true;
    }
    return false;
  }

  @override
  bool moveNext() {
    if (_state == null) {
      return false;
    }

    Object? pendingException;
    StackTrace? pendingStackTrace;
    while (true) {
      // Primeiro o iterador aninhado de um `yield*` (se houver).
      final iterator = _yieldStarIterator;
      if (iterator != null) {
        try {
          if (iterator.moveNext()) {
            _current = iterator.current;
            return true;
          }
        } catch (exception, stackTrace) {
          pendingException = exception;
          pendingStackTrace = stackTrace;
        }
        _yieldStarIterator = null;
      }

      try {
        // Retoma o corpo corrente até o próximo elemento.
        final corpo = _state!;
        final bool hasMore = (pendingException == null
            ? corpo(this, _SUCESSO, null)
            : corpo(this, _ERRO,
                _ErroAssincrono(pendingException, pendingStackTrace!))) as bool;
        pendingException = null;
        pendingStackTrace = null;
        if (!hasMore) {
          if (_handleSyncStarMethodCompletion()) {
            continue;
          }
          return false;
        }
      } catch (exception, stackTrace) {
        pendingException = exception;
        pendingStackTrace = stackTrace;
        if (_handleSyncStarMethodCompletion()) {
          continue;
        }
        rethrow;
      }

      // `yield*` de um iterável.
      final iterable = _yieldStarIterable;
      if (iterable != null) {
        _yieldStarIterable = null;
        _current = null;
        if (iterable is _SyncStarIterable) {
          // `yield*` de outro sync*: o corpo dele passa a ser o corrente, e
          // este fica na pilha até ele terminar.
          final stack = (_stack ??= []);
          stack.add(_state!);
          _state = unsafeCast<_SyncStarIterable>(iterable)._novoCorpo();
        } else {
          try {
            _yieldStarIterator = iterable.iterator;
          } catch (exception, stackTrace) {
            pendingException = exception;
            pendingStackTrace = stackTrace;
          }
        }
        continue;
      }

      return true;
    }
  }
}

// ─── async* ─────────────────────────────────────────────────────────────────
//
// O controlador é o da VM, sem mudança de comportamento (a ordem de eventos e
// microtarefas é a dela). Onde a VM retoma o `_SuspendState`, aqui se chama o
// corpo transformado: `asyncStarBody(cancelado)` vira
// `corpo(_SUCESSO, cancelado)`. O lowering emite, como o compilador da VM:
//
// * no começo do corpo e depois de cada `yield`: se `cancelado`, `return`;
// * `yield e`: `if (_asyncStarAdd(c, e)) return;` e suspende;
// * `yield* s`: `if (_asyncStarAddStream(c, s)) return;` e suspende;
// * `return`: `_asyncStarReturn(c)`; exceção não capturada:
//   `_asyncStarErro(c, e, s)`.

@pragma("vm:entry-point")
class _AsyncStarStreamController<T> {
  StreamController<T> controller;
  void Function(Object?)? asyncStarBody;
  bool isAdding = false;
  bool onListenReceived = false;
  bool isScheduled = false;
  bool isSuspendedAtYield = false;
  _Future? cancellationFuture = null;

  Stream<T> get stream {
    return controller.stream;
  }

  void runBody() {
    isScheduled = false;
    isSuspendedAtYield = false;
    asyncStarBody!(!controller.hasListener);
  }

  void scheduleGenerator() {
    if (isScheduled || controller.isPaused || isAdding) {
      return;
    }
    isScheduled = true;
    scheduleMicrotask(runBody);
  }

  // Acrescenta o evento ao stream. Devolve true se o gerador deve terminar.
  bool add(T event) {
    if (!onListenReceived) throw StateError("yield before stream is listened to");
    if (isSuspendedAtYield) throw StateError("unexpected yield");
    controller.add(event);
    if (!controller.hasListener) {
      return true;
    }

    scheduleGenerator();
    isSuspendedAtYield = true;
    return false;
  }

  // Acrescenta os elementos de `stream`; o gerador volta a ser agendado
  // quando todos forem consumidos. Devolve true se o gerador deve terminar.
  bool addStream(Stream<T> stream) {
    if (!onListenReceived) throw StateError("yield before stream is listened to");
    if (!controller.hasListener) {
      return true;
    }

    isAdding = true;
    final whenDoneAdding = controller.addStream(stream, cancelOnError: false);
    final self = this;
    whenDoneAdding.then((_) {
      self.isAdding = false;
      self.scheduleGenerator();
      if (!self.isScheduled) self.isSuspendedAtYield = true;
    });

    return false;
  }

  void addError(Object error, StackTrace stackTrace) {
    final future = cancellationFuture;
    if ((future != null) && future._mayComplete) {
      future._completeError(error, stackTrace);
      return;
    }
    if (!controller.hasListener) return;
    controller.addError(error, stackTrace);
  }

  close() {
    final future = cancellationFuture;
    if ((future != null) && future._mayComplete) {
      future._completeWithValue(null);
    }
    controller.close();
  }

  _AsyncStarStreamController() : controller = new StreamController(sync: true) {
    controller.onListen = this.onListen;
    controller.onResume = this.onResume;
    controller.onCancel = this.onCancel;
  }

  onListen() {
    assert(!onListenReceived);
    onListenReceived = true;
    scheduleGenerator();
  }

  onResume() {
    if (isSuspendedAtYield) {
      scheduleGenerator();
    }
  }

  onCancel() {
    if (controller.isClosed) {
      return null;
    }
    if (cancellationFuture == null) {
      cancellationFuture = new _Future();
      // Só retoma o gerador suspenso num `yield`; o cancelamento não afeta um
      // gerador suspenso num `await`.
      if (isSuspendedAtYield) {
        scheduleGenerator();
      }
    }
    return cancellationFuture;
  }
}

/// Cria o controlador de uma chamada de função `async*`.
@pragma("vm:entry-point", "call")
_AsyncStarStreamController<T> _makeAsyncStarController<T>() {
  return _AsyncStarStreamController<T>();
}

/// Liga o corpo ao controlador e devolve o stream (o gerador só roda quando
/// alguém escuta).
@pragma("vm:entry-point", "call")
Stream _asyncStarStart(
    _AsyncStarStreamController controlador, _WrappedAsyncBody corpo) {
  controlador.asyncStarBody = (cancelado) {
    corpo(_SUCESSO, cancelado);
  };
  return controlador.stream;
}

/// `yield e` num corpo `async*`: true se o gerador deve terminar.
@pragma("vm:entry-point", "call")
bool _asyncStarAdd(_AsyncStarStreamController controlador, Object? valor) {
  return controlador.add(valor);
}

/// `yield* s` num corpo `async*`: true se o gerador deve terminar.
@pragma("vm:entry-point", "call")
bool _asyncStarAddStream(_AsyncStarStreamController controlador, Stream s) {
  return controlador.addStream(s);
}

/// O fim de um corpo `async*`.
@pragma("vm:entry-point", "call")
void _asyncStarReturn(_AsyncStarStreamController controlador) {
  controlador.close();
}

/// A exceção não capturada de um corpo `async*`.
@pragma("vm:entry-point", "call")
void _asyncStarErro(
    _AsyncStarStreamController controlador, Object erro, StackTrace rastro) {
  controlador.addError(erro, rastro);
  controlador.close();
}

// ─── await for ──────────────────────────────────────────────────────────────
//
// `await for (x in s) corpo` é baixado como o kernel da VM o desaçucara:
//
//     final it = StreamIterator(s);
//     try {
//       while (await it.moveNext()) { x = it.current; corpo }
//     } finally {
//       if (it._subscription != null) await it.cancel();
//     }

/// O `StreamIterator` de um `await for`.
@pragma("vm:entry-point", "call")
StreamIterator<T> _awaitForIterador<T>(Stream<T> s) {
  return StreamIterator<T>(s);
}

/// Se o iterador do `await for` ainda tem inscrição (então o `finally`
/// cancela; sem inscrição não há o que cancelar, e não se espera nada).
@pragma("vm:entry-point", "call")
bool _awaitForAtivo(StreamIterator iterador) {
  return iterador is _StreamIterator && iterador._subscription != null;
}
