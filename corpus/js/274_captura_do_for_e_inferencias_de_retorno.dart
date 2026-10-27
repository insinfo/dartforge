// A closure in a `for` initializer captures the variables of the first
// iteration (a JS `for` would give it a separate copy). A local `async`
// function infers its return type from every `return`, also those of a branch
// that is never taken, and a written `dynamic` return type stays `dynamic`.
// `dynamic` and `void` against `T?` solve `T` to `Object`. `await` on a type
// variable promoted to a future type awaits it. A closure assigned to a
// promoted local is inferred against the declared type. An implicit
// instantiation in a constant solves to `Never`.
import 'dart:async';

class A {}

class B extends A {}

extension Compact<T extends Object> on List<T?> {
  List<T> compact() => [
        for (var e in this)
          if (e != null) e
      ];
}

R pick<T, R>(T _) => throw 'uncalled';

class Holder<T> {
  final Object fn;
  const Holder(T Function(T) this.fn);
}

Holder<T> holder<T>() => const Holder(pick);

Future<void> awaitPromoted<X>(X x) async {
  if (x is Future<A>) {
    var v = await x;
    print('awaited ${v.runtimeType}');
  }
}

Future<void> main() async {
  var closures = <int Function()>[];
  for (var i = 0, first = () => i; i < 3; i++) {
    i += 1;
    closures.add(first);
    closures.add(() => i);
  }
  print(closures.map((f) => f()).toList());

  var values = <int>[];
  for (var i = 0, f = () => i; i < 6; i += 2) {
    values.add(f());
    if (i == 2) continue;
    values.add(i);
  }
  print(values);

  inferred() async {
    if (false) {
      return 0;
    } else {
      return A();
    }
  }

  dynamic declared() async => B();
  print(inferred is Future<Object> Function());
  print(inferred is Future<A> Function());
  print(declared is Future<dynamic> Function());
  print(declared is dynamic Function());

  const dynamic o = 42;
  var compacted = [o].compact();
  print(compacted is List<Object>);
  compacted.add('text');
  print(compacted);

  await awaitPromoted<Object>(Future<A>.value(B()));

  List<Object> Function() g;
  List<int> ints = [1];
  g = () => ints;
  print(g is List<int> Function());
  dynamic d = ints;
  g = () {
    return d;
  };
  print(g is List<int> Function());
  print(g is List<Object> Function());

  print(holder<int>().fn is Never Function(Never));
}
