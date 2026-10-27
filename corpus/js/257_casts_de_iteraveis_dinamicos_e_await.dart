// A dynamic spread or for-in iterable is cast to Iterable (or Map) and each
// element to the literal's or loop variable's type. An async arrow body and a
// return in an async function check the value against FutureOr<T>. await of
// a value whose static type is not a future only awaits a Future<flatten(S)>.
// await for works inside a collection literal.
import 'dart:async';

void expectTypeError(String label, Object? Function() f) {
  try {
    f();
    print('$label: no error');
  } on TypeError {
    print('$label: TypeError');
  }
}

Stream<int> count(int n) async* {
  for (var i = 0; i < n; i++) {
    yield i;
  }
}

Future<void> main() async {
  dynamic notIterable = 3;
  dynamic strings = <String>['s'];
  dynamic map = {1: 'one'};
  expectTypeError('spread non-iterable', () => <int>[...notIterable]);
  expectTypeError('spread wrong element', () => <int>[...strings]);
  expectTypeError('spread map in list', () => <int>[...map]);
  expectTypeError('spread wrong value', () => <int, int>{...map});
  expectTypeError('for-in non-iterable', () => <int>[for (var i in notIterable) i]);
  expectTypeError('for-in wrong variable', () => [for (int i in strings) i]);
  expectTypeError('for statement', () {
    for (int i in strings) {
      print(i);
    }
    return null;
  });
  dynamic ints = [1, 2];
  print(<int>[...ints, for (int i in ints) i * 10]);

  var inc = (dynamic p) async => p + 1;
  print(await inc(1));
  Future<int> viaReturn(dynamic f) async {
    return f;
  }

  print(await viaReturn(Future<int>.value(5)));

  final Object o = Future<Object?>.value('kept');
  var o2 = await o;
  print(o2 is Future<Object?>);
  final FutureOr<Object> x = Future<Object>.value('awaited');
  print(await x);
  Future<void> generic<X extends Object>(X value) async {
    var v = await value;
    print(v is Future);
  }

  await generic(Future<Object?>.value(null));
  await generic(Future<int>.value(1));
  print([await for (var i in count(3)) i]);
}
