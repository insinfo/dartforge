// `async` bodies lowered to a state machine in production
// (`emit_js/src/async_machine.rs`): every control structure around an
// `await`, the evaluation order around it, errors, `finally`, and the
// order of the event loop (microtasks, timers, `then` callbacks).
import 'dart:async';

final log = <String>[];

void note(String s) => log.add(s);

Future<int> value(int v) async => v;

Future<int> later(int v) => Future(() => v);

Future<int> failing(String m) async {
  await null;
  throw StateError(m);
}

int sideEffect(String tag, int v) {
  note('eval $tag');
  return v;
}

class Counter {
  int n = 0;
  int get next => ++n;
  Future<int> bump(int by) async {
    n += await value(by);
    return n;
  }

  Future<String> describe() async => 'Counter(${await value(n)})';
}

Future<void> straightLine() async {
  final a = await value(1);
  final b = await later(2);
  final c = await 3; // a non-future
  final Object? d = await null;
  note('straight $a $b $c $d');
}

Future<void> evaluationOrder() async {
  var x = 1;
  Future<int> bumpX() async {
    x = 100;
    return 5;
  }

  // `x` is read before the await, as on the VM.
  final r = x + await bumpX();
  note('order $r $x');
  var s = 10;
  s += await value(sideEffect('rhs', 2));
  note('compound $s');
  final args = [sideEffect('a', 1), await value(sideEffect('b', 2)), sideEffect('c', 3)];
  note('args $args');
  final m = {'k': await value(4), 'j': sideEffect('j', 5)};
  note('map $m');
  final str = 'v=${await value(6)} w=${sideEffect('w', 7)}';
  note(str);
  final cond = (await value(0)) == 0 ? await value(8) : sideEffect('no', 9);
  note('cond $cond');
  final and = x > 50 && await value(1) == 1;
  final or = x < 50 || await value(2) == 2;
  final Object? nul = null;
  final co = nul ?? await value(11);
  note('logic $and $or $co');
  final list = [for (var i = 0; i < 3; i++) await value(i * 2), if (await value(1) == 1) 99];
  note('collection $list');
  final c = Counter();
  note('method ${await c.bump(3)} ${await c.bump(4)} ${c.next} ${await c.describe()}');
}

Future<int> loops() async {
  var total = 0;
  for (var i = 0; i < 4; i++) {
    if (i == 1) continue;
    total += await value(i);
    if (total > 100) break;
  }
  var j = 0;
  while (await value(j) < 3) {
    j++;
  }
  var k = 0;
  do {
    k += await value(2);
  } while (k < 7);
  outer:
  for (final a in [1, 2, 3]) {
    for (final b in <int>{10, 20, 30}) {
      if (b == 20) continue outer;
      if (a == 3) break outer;
      total += await value(a * b);
    }
  }
  final iterable = Iterable.generate(3, (i) => i + 1);
  for (final v in iterable) {
    total += await value(v);
  }
  note('loops $total $j $k');
  return total;
}

Future<String> switches(Object o) async {
  switch (o) {
    case 1:
      await null;
      return 'one';
    case 'two':
      return 'two ${await value(2)}';
    case int n when n > 5:
      final r = await value(n);
      return 'big $r';
    default:
      await null;
  }
  var s = '';
  switch (await value(3)) {
    case 3:
      s = 'three';
    case 4:
      s = 'four';
  }
  return 'default $s';
}

Future<String> tryCatchFinally(int mode) async {
  final steps = <String>[];
  try {
    steps.add('try');
    if (mode == 0) await failing('boom');
    if (mode == 1) throw ArgumentError('sync');
    if (mode == 2) return 'early ${await value(2)}';
    steps.add('after');
  } on StateError catch (e) {
    steps.add('state ${e.message}');
    await null;
  } on ArgumentError catch (e, st) {
    steps.add('arg ${e.message} ${st is StackTrace}');
  } finally {
    steps.add('finally ${await value(mode)}');
  }
  return steps.join(',');
}

Future<String> finallyOverrides() async {
  try {
    return 'from try';
  } finally {
    await null;
    note('finally ran before the completion');
  }
}

Future<int> breakThroughFinally() async {
  var n = 0;
  for (var i = 0; i < 5; i++) {
    try {
      try {
        if (i == 3) break;
        if (i == 1) continue;
        n += await value(10);
      } finally {
        n += 1;
        await null;
      }
    } finally {
      n += 100;
    }
  }
  return n;
}

Future<void> rethrows() async {
  try {
    try {
      await failing('inner');
    } catch (e) {
      note('caught ${(e as StateError).message}');
      await null;
      rethrow;
    }
  } catch (e) {
    note('outer ${(e as StateError).message}');
  }
  try {
    await (() async {
      try {
        throw 'thrown';
      } finally {
        note('inner finally');
      }
    })();
  } catch (e) {
    note('closure $e');
  }
  try {
    try {
      await value(1);
      throw 'first';
    } finally {
      // The finally's own error replaces the pending one.
      throw 'second';
    }
  } catch (e) {
    note('replaced $e');
  }
}

Future<int> errorBeforeFirstAwait(bool fail) async {
  if (fail) throw StateError('sync part');
  return await value(1);
}

Future<void> closures() async {
  final fs = <int Function()>[];
  for (var i = 0; i < 3; i++) {
    await null;
    fs.add(() => i); // per-iteration binding: the generator stays
  }
  note('closures ${fs.map((f) => f()).toList()}');
  var shared = 0;
  final inc = () async {
    shared += await value(1);
  };
  await inc();
  await inc();
  note('shared $shared');
}

Future<void> capturedParameter(int x) async {
  // The closure reads the enclosing function's parameter before its
  // await; the enclosing function changes it during the await.
  final reader = () async {
    final r = x + await later(1);
    note('captured parameter $r $x');
  }();
  x = 1000;
  await reader;
}

Future<void> eventLoop() async {
  final order = <String>[];
  scheduleMicrotask(() => order.add('micro 1'));
  Timer.run(() => order.add('timer'));
  final f = value(1).then((v) => order.add('then $v'));
  order.add('sync');
  await null;
  order.add('after await null');
  scheduleMicrotask(() => order.add('micro 2'));
  await f;
  order.add('after then');
  await Future.delayed(Duration.zero);
  order.add('after delayed');
  final completer = Completer<int>();
  final waiter = (() async {
    order.add('waiting');
    final v = await completer.future;
    order.add('woke $v');
    return v;
  })();
  order.add('before complete');
  completer.complete(7);
  order.add('after complete');
  await waiter;
  final both = await Future.wait([value(1), later(2), value(3)]);
  order.add('wait $both');
  note(order.join(' | '));
}

Stream<int> numbers() async* {
  for (var i = 0; i < 3; i++) {
    yield await value(i);
  }
}

Future<void> streams() async {
  var sum = 0;
  await for (final n in numbers()) {
    sum += n;
    if (n == 1) await null;
  }
  note('stream $sum');
}

Future<void> main() async {
  await straightLine();
  await evaluationOrder();
  await loops();
  for (final o in [1, 'two', 9, 0]) {
    note(await switches(o));
  }
  for (var mode = 0; mode < 4; mode++) {
    note(await tryCatchFinally(mode));
  }
  note(await finallyOverrides());
  note('break ${await breakThroughFinally()}');
  await rethrows();
  final early = errorBeforeFirstAwait(true);
  note('called');
  try {
    await early;
  } catch (e) {
    note('early ${(e as StateError).message}');
  }
  note('late ${await errorBeforeFirstAwait(false)}');
  await closures();
  await capturedParameter(5);
  await eventLoop();
  await streams();
  runZonedGuarded(() async {
    await failing('zoned');
  }, (e, st) => note('zone ${(e as StateError).message}'));
  await Future.delayed(const Duration(milliseconds: 1));
  log.forEach(print);
}
