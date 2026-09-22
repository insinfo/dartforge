// Convertido de tests/conformance/modules/async23 (módulo antigo do corpus de conformidade).
import 'dart:async';

Future<int> step() async {
  print('step-start');
  await Future<int>.value(0);
  print('step-end');
  return 7;
}

Future<void> arrowCompletion() async => Future<int>.delayed(Duration.zero, () { print('arrow-value'); return 1; });

Future<int> adopt() async => Future<int>.value(8);
Future<Future<int>> nested() async => Future<int>.value(9);

class Counter {
  int value = 0;
  Future<int> increment() async {
    await Future<void>.value();
    value = value + 1;
    return value;
  }
}

Future<void> main() async {
  print('start');
  var cancelled = Timer(Duration.zero, () => print('cancelled-error'));
  print(cancelled.isActive);
  print(cancelled.tick);
  cancelled.cancel();
  cancelled.cancel();
  print(cancelled.isActive);
  print(cancelled.tick);
  var timer = Timer(Duration.zero, () => print('timer'));
  scheduleMicrotask(() => print('micro-one'));
  var pending = step();
  scheduleMicrotask(() => print('micro-two'));
  print('sync-end');
  print(await pending);
  print(timer.tick);
  await Future<void>.delayed(Duration.zero, () => print('delayed'));
  print(timer.tick);
  print(timer.isActive);
  print(await adopt());
  var inner = await nested();
  print(inner is Future<int>);
  print(await inner);
  var boxed = Future<Future<int>>.value(Future<int>.value(10));
  var unboxed = await boxed;
  print(unboxed is Future<int>);
  print(await unboxed);
  var adoptedObject = await Future<Object>.value(Future<int>.value(11));
  print(adoptedObject is int);
  print(adoptedObject as int);
  var typed = Future<int>.value(12);
  print(typed is Future<int>);
  print(typed is Future<Object>);
  print(typed is Future<String>);
  var count = 20;
  var callback = () async {
    await Future<void>.value();
    count = count + 1;
    return count;
  };
  print(await callback());
  print(count);
  var counter = Counter();
  print(await counter.increment());
  print(counter.value);
  print(await Future<int>.delayed(Duration(milliseconds: 1), () => 30));
  print(Duration(seconds: 1, milliseconds: 2).inMilliseconds);
  await arrowCompletion();
  print('arrow-after');
  print('done');
}
