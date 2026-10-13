// Patch of `dart:async` for the DartForge native backend (overlay
// `native_sdk/`, docs/NATIVO-PLANO.md §7). Replaces
// `_internal/vm/lib/async_patch.dart` of the `vm` section of SDK 3.6.2.
//
// Why not the VM's: the VM's `async`/`sync*`/`async*` suspends the machine
// frame (`_SuspendState._resume`/`_clone`, intrinsics with no fallback), which
// is incompatible with the native backend's explicit shadow stack. The model
// here is dart2js's (`_internal/js_runtime/lib/async_patch.dart`): the `async`
// body becomes a state machine (P6), the frame lives in the heap, and each
// `await` registers the continuation `body(statusCode, res)`. The members below
// are dart2js's, without `JS(...)`: the exception loop dart2js writes in JS
// (`_wrapJsFunctionForAsync`) is written in Dart in `_wrapBody`.
//
// All the source `dart:async` requires from a patch is this (its four
// `external`s): `_trySetStackTrace`, `Timer._createTimer`,
// `Timer._createPeriodicTimer` (`timer_patch.dart`) and
// `_AsyncRun._scheduleImmediate` (`schedule_microtask_patch.dart`). The rest is
// the support the `async` lowering calls.

import "dart:_internal" show patch, unsafeCast;

part "schedule_microtask_patch.dart";
part "timer_patch.dart";

@patch
@pragma("vm:external-name", "Error_trySetStackTrace")
external void _trySetStackTrace(Object error, StackTrace stackTrace);

/// Status codes of the continuation of an `async` body (those of dart2js's
/// `dart:_async_status_codes`).
const int _SUCCESS = 0;
const int _ERROR = 1;

/// The continuation of an `async` body turned into a state machine:
/// `body(_SUCCESS, val)` resumes with the value of the `await`; `body(_ERROR,
/// err)` resumes by throwing `err` at the `await` (§4.2 of the plan).
typedef _WrappedAsyncBody = void Function(int errorCode, dynamic result);

/// The (error, stack trace) pair the continuation receives with `_ERROR`.
final class _DfAsyncError {
  final Object err;
  final StackTrace trace;
  _DfAsyncError(this.err, this.trace);
}

class _AsyncAwaitCompleter<T> implements Completer<T> {
  final _future = _Future<T>();
  bool isSync;

  _AsyncAwaitCompleter() : isSync = false;

  void complete([FutureOr<T>? value]) {
    // Every path requires that, if the value is null, `null as T` passes.
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

/// Creates the `Completer` of an `async` function.
@pragma("vm:entry-point", "call")
Completer<T> _makeAsyncAwaitCompleter<T>() {
  return _AsyncAwaitCompleter<T>();
}

/// Starts the body of an `async` function synchronously and returns its
/// `Future`.
///
/// The helpers marked `dartforge:hidden-frame` (and their closures) stay out
/// of the symbolic stack trace: the VM does not have those frames
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14).
@pragma("vm:entry-point", "call")
@pragma("dartforge:hidden-frame")
dynamic _asyncStartSync(
    _WrappedAsyncBody bodyFunction, _AsyncAwaitCompleter completer) {
  bodyFunction(_SUCCESS, null);
  completer.isSync = true;
  return completer.future;
}

/// `p.loadLibrary()` of a `deferred` import: the library is already in the
/// compiled program.
@pragma("vm:entry-point", "call")
Future<Null> _dartforgeLoadLibrary() => new _Future<Null>.value(null);

/// The `await` of an `async` function: registers the continuation on the object.
@pragma("vm:entry-point", "call")
@pragma("dartforge:hidden-frame")
dynamic _asyncAwait(dynamic object, _WrappedAsyncBody bodyFunction) {
  _awaitOnObject(object, bodyFunction);
}

/// The `return` of an `async` function.
@pragma("vm:entry-point", "call")
@pragma("dartforge:hidden-frame")
dynamic _asyncReturn(dynamic object, Completer completer) {
  completer.complete(object);
}

/// The uncaught exception of an `async` function (the by-value model: the
/// body ends with the pending exception, which arrives here as a value).
@pragma("vm:entry-point", "call")
@pragma("dartforge:hidden-frame")
dynamic _asyncRethrow(Object error, StackTrace stackTrace, Completer completer) {
  completer.completeError(error, stackTrace);
}

/// Awaits `object`: if it is a `Future`, registers the continuation on it;
/// otherwise, on a `_Future` already completed with it (the microtask order is
/// the source's).
///
/// `vm:awaiter-link`: the closures that capture `bodyFunction` carry the
/// symbolic stack trace to the awaiter (the VM's `TryGetAwaiterLink`).
@pragma("dartforge:hidden-frame")
void _awaitOnObject(object, @pragma('vm:awaiter-link') _WrappedAsyncBody bodyFunction) {
  FutureOr<dynamic> Function(dynamic) thenCallback =
      (result) => bodyFunction(_SUCCESS, result);

  Function errorCallback = (Object error, StackTrace stackTrace) {
    bodyFunction(_ERROR, _DfAsyncError(error, stackTrace));
  };

  if (object is _Future) {
    // The continuation was already registered in the zone (`_wrapBody`).
    object._thenAwait(thenCallback, errorCallback);
  } else if (object is Future) {
    object.then(thenCallback, onError: errorCallback);
  } else {
    _Future future = _Future().._setValue(object);
    future._thenAwait(thenCallback, errorCallback);
  }
}

/// Registers the body in the current zone and guards it: if resuming throws,
/// the body is called again with `_ERROR` — the loop dart2js writes in JS in
/// `_wrapJsFunctionForAsync`.
@pragma("vm:entry-point", "call")
@pragma("dartforge:hidden-frame")
_WrappedAsyncBody _wrapBody(@pragma('vm:awaiter-link') _WrappedAsyncBody body) {
  void guarded(int code, dynamic res) {
    while (true) {
      try {
        body(code, res);
        break;
      } catch (e, s) {
        res = _DfAsyncError(e, s);
        code = _ERROR;
      }
    }
  }

  return Zone.current.registerBinaryCallback<void, int, dynamic>(guarded);
}

// ─── sync* ──────────────────────────────────────────────────────────────────
//
// The model is the VM's (`_internal/vm/lib/async_patch.dart`, `_SyncStarIterable`
// and `_SyncStarIterator`), with the state machine in place of the
// `_SuspendState`: calling the `sync*` function runs nothing and returns the
// iterable; each `iterator` starts the body from the beginning, with a new
// frame (`_newBody`, the equivalent of the VM's `_stateAtStart._clone()`).
//
// The body is `body(iter, code, val)` and returns whether there are more
// elements: `yield e` stores `_current` and returns `true`; `yield* e` stores
// `_yieldStarIterable` and returns `true`; the end of the body returns
// `false`. An exception leaves the body as from any function, and `moveNext`
// handles it like the VM; `code == _ERROR` resumes by throwing the
// `_DfAsyncError` at the `yield*` (the nested iterator's exception).

/// The body of a `sync*` function turned into a state machine:
/// `bool body(_SyncStarIterator iter, int code, Object? val)`. It stays a
/// `Function` (called dynamically) because the closure the lowering creates
/// does not carry the reified function type, and a test against the exact type would fail.
typedef _SyncStarBody = Function;

/// Creates the iterable of a `sync*` function call.
@pragma("vm:entry-point", "call")
_SyncStarIterable<T> _makeSyncStarIterable<T>(Function fresh) {
  return _SyncStarIterable<T>(fresh);
}

/// `yield e` in a `sync*` body.
@pragma("vm:entry-point", "call")
bool _syncStarYield(_SyncStarIterator iter, Object? val) {
  iter._current = val;
  return true;
}

/// `yield* e` in a `sync*` body.
@pragma("vm:entry-point", "call")
bool _syncStarYieldStar(_SyncStarIterator iter, Iterable iterableArg) {
  iter._yieldStarIterable = iterableArg;
  return true;
}

class _SyncStarIterable<T> extends Iterable<T> {
  // `_SyncStarBody Function()`, without the exact type (see `_SyncStarBody`).
  final Function _newBody;

  _SyncStarIterable(this._newBody);

  Iterator<T> get iterator {
    return _SyncStarIterator<T>(_newBody());
  }
}

class _SyncStarIterator<T> implements Iterator<T> {
  _SyncStarBody? _state;
  Iterator<T>? _yieldStarIterator;

  // Stack of the sync* bodies suspended in a `yield*` of another sync*.
  List<_SyncStarBody>? _stack;

  // The body stores `_current` or `_yieldStarIterable` before suspending.
  T? _current;
  Iterable<T>? _yieldStarIterable;

  @override
  T get current => _current as T;

  _SyncStarIterator(_SyncStarBody state) : _state = state;

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
      // First the nested iterator of a `yield*` (if any).
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
        // Resumes the current body until the next element.
        final body = _state!;
        final bool hasMore = (pendingException == null
            ? body(this, _SUCCESS, null)
            : body(this, _ERROR,
                _DfAsyncError(pendingException, pendingStackTrace!))) as bool;
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

      // `yield*` of an iterable.
      final iterable = _yieldStarIterable;
      if (iterable != null) {
        _yieldStarIterable = null;
        _current = null;
        if (iterable is _SyncStarIterable) {
          // `yield*` of another sync*: its body becomes the current one, and this
          // one stays on the stack until it ends.
          final stack = (_stack ??= []);
          stack.add(_state!);
          _state = unsafeCast<_SyncStarIterable>(iterable)._newBody();
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
// The controller is the VM's, with no change of behavior (the order of events
// and microtasks is its order). Where the VM resumes the `_SuspendState`, here
// the transformed body is called: `asyncStarBody(canceled)` becomes
// `body(_SUCCESS, canceled)`. The lowering emits, like the VM's compiler:
//
// * at the start of the body and after each `yield`: if `canceled`, `return`;
// * `yield e`: `if (_asyncStarAdd(c, e)) return;` and suspends;
// * `yield* s`: `if (_asyncStarAddStream(c, s)) return;` and suspends;
// * `return`: `_asyncStarReturn(c)`; uncaught exception:
//   `_asyncStarError(c, e, s)`.

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

  // Adds the event to the stream. Returns true if the generator must end.
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

  // Adds the elements of `stream`; the generator is scheduled again when all
  // of them are consumed. Returns true if the generator must end.
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
      // Only resumes a generator suspended in a `yield`; cancellation does not
      // affect a generator suspended in an `await`.
      if (isSuspendedAtYield) {
        scheduleGenerator();
      }
    }
    return cancellationFuture;
  }
}

/// Creates the controller of an `async*` function call.
@pragma("vm:entry-point", "call")
_AsyncStarStreamController<T> _makeAsyncStarController<T>() {
  return _AsyncStarStreamController<T>();
}

/// Binds the body to the controller and returns the stream (the generator only
/// runs when someone listens).
@pragma("vm:entry-point", "call")
Stream _asyncStarStart(_AsyncStarStreamController asyncController,
    @pragma('vm:awaiter-link') _WrappedAsyncBody body) {
  asyncController.asyncStarBody = (canceled) {
    body(_SUCCESS, canceled);
  };
  return asyncController.stream;
}

/// `yield e` in an `async*` body: true if the generator must end.
@pragma("vm:entry-point", "call")
bool _asyncStarAdd(_AsyncStarStreamController asyncController, Object? val) {
  return asyncController.add(val);
}

/// `yield* s` in an `async*` body: true if the generator must end.
@pragma("vm:entry-point", "call")
bool _asyncStarAddStream(_AsyncStarStreamController asyncController, Stream s) {
  return asyncController.addStream(s);
}

/// The end of an `async*` body.
@pragma("vm:entry-point", "call")
void _asyncStarReturn(_AsyncStarStreamController asyncController) {
  asyncController.close();
}

/// The uncaught exception of an `async*` body.
@pragma("vm:entry-point", "call")
void _asyncStarError(
    _AsyncStarStreamController asyncController, Object err, StackTrace trace) {
  asyncController.addError(err, trace);
  asyncController.close();
}

// ─── await for ──────────────────────────────────────────────────────────────
//
// `await for (x in s) body` is lowered the way the VM's kernel desugars it:
//
//     final it = StreamIterator(s);
//     try {
//       while (await it.moveNext()) { x = it.current; body }
//     } finally {
//       if (it._subscription != null) await it.cancel();
//     }

/// The `StreamIterator` of an `await for`.
@pragma("vm:entry-point", "call")
StreamIterator<T> _awaitForIterator<T>(Stream<T> s) {
  return StreamIterator<T>(s);
}

/// Whether the `await for` iterator still has a subscription (then the
/// `finally` cancels; without a subscription there is nothing to cancel, and nothing is awaited).
@pragma("vm:entry-point", "call")
bool _awaitForActive(StreamIterator iter) {
  return iter is _StreamIterator && iter._subscription != null;
}
