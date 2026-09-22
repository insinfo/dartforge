import 'dart:async';

Future<int> dobro(int x) async {
  await Future.delayed(Duration(milliseconds: 1));
  return x * 2;
}
Future<void> nada() async { print('nada'); }
Future<String> fut() => Future.value('v');
Stream<int> conta(int n) async* {
  for (var i = 0; i < n; i++) {
    yield i;
  }
  yield* Stream.fromIterable([100, 200]);
}
Iterable<int> pares(int n) sync* {
  for (var i = 0; i < n; i++) {
    if (i.isEven) yield i;
  }
  yield* [7, 8];
}
Future<int> soma() async {
  var t = 0;
  await for (var v in conta(3)) {
    t += v;
  }
  return t;
}
Future<void> erro() async {
  throw StateError('x');
}
class A {
  Future<int> m() async => 5;
  Stream<int> s() async* { yield 1; }
}
Future<void> main() async {
  print(await dobro(4));
  await nada();
  print(await fut());
  print(pares(6).toList());
  print(await soma());
  try {
    await erro();
  } catch (e) {
    print('caught ${e.runtimeType}');
  }
  var l = await Future.wait([dobro(1), dobro(2)]);
  print(l);
  var c = Completer<int>();
  c.future.then((v) => print('then $v'));
  c.complete(9);
  await c.future;
  print(await A().m());
  await for (final v in A().s()) print(v);
  var f = () async => 3;
  print(await f());
  var g = () async { return 'g'; };
  print(await g());
  var s = conta(2).map((e) => e * 3);
  print(await s.toList());
  print(await conta(2).length);
  await Future.forEach<int>([1, 2], (e) async { print('fe $e'); });
  Future<int>.delayed(Duration.zero, () => 1).then((v) { print('delayed $v'); });
  await Future(() => print('micro'));
  var results = <int>[];
  for (var i in [1, 2, 3]) { results.add(await dobro(i)); }
  print(results);
  print(await Future.value(1) + await Future.value(2));
  scheduleMicrotask(() => print('mt'));
  await Future.delayed(Duration.zero);
  var sc = StreamController<String>();
  sc.stream.listen((e) => print('ev $e'), onDone: () => print('done'));
  sc.add('a');
  sc.add('b');
  await sc.close();
  await Future.delayed(Duration.zero);
  print(await pares(3).map((e) async => e).first);
  print('fim');
}
