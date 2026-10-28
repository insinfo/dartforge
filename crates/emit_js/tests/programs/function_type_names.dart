// Printed function types as on the VM: named parameters by their source
// names (the production profile shortens their keys) and in name order, and
// the type parameters of generic function types named `Y0`, `Y1`… and, inside
// enclosing type parameters, `F<n>Y0`… (DDC and dart2js print `T1`).

void withStart({int? start}) {}

int optional(int a, [String? b]) => a;

T identity<T>(T x) => x;

U second<T, U>(T a, U b) => b;

class Counter {
  // Declared in name order: the VM prints a tear-off's named parameters in
  // declaration order, DDC and dart2js by name.
  void count({required int count, String? label, bool? quiet}) {}
  R apply<R>(R Function({int? start}) f) => f();
}

void main() {
  print(withStart.runtimeType);
  print(optional.runtimeType);
  print(identity.runtimeType);
  print(second.runtimeType);
  print(Counter().count.runtimeType);
  print(Counter().apply.runtimeType);
  Object closure = (int x, {int? start, bool? flag}) => x;
  print(closure.runtimeType);
  print(closure is int Function(int, {int? start}));
  print(closure is int Function(int, {String? start}));
  print(((<A>(A a, B Function<B>(B, A) f) => a)).runtimeType);
  print((<X extends num>(X x) => x).runtimeType);
  print((<X, Y extends List<X>>(X x, Y y) => x).runtimeType);
  print((<X>(X Function<Z>(Z, X) f, {X? start}) => f).runtimeType);
  dynamic d = withStart;
  d(start: 1);
  try {
    d(begin: 1);
  } on NoSuchMethodError catch (e) {
    print('no begin: ${e.toString().contains('begin')}');
  }
  print('done');
}
