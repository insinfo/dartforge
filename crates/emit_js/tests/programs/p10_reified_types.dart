// Reified type arguments that come from inference (docs/INFERENCIA-JS-ALINHAMENTO.md §4):
// the instantiation of generic calls, collection literals, constructors and
// closures is observable through `is`, `as`, `runtimeType` and the printed `T`.
import 'dart:async';
import 'dart:math' as math;

T id<T>(T x) => x;
String nomeDe<T>(T x) => '$T';
T primeiro<T>(List<T> xs) {
  print('primeiro: List<int>? ${xs is List<int>}; T = $T');
  return xs.first;
}

List<T> singleton<T>(T x) => [x];
T first<T>(List<T> xs) => xs.first;
T maior<T extends Comparable<Object>>(T a, T b) => a.compareTo(b) >= 0 ? a : b;

class Caixa<T> {
  final T v;
  Caixa(this.v);
  String get tipo => '$T';
}

class Pilha<E> {
  final List<E> _itens = [];
  void push(E e) => _itens.add(e);
  List<E> get itens => List.unmodifiable(_itens);
}

class Base {}

class Child extends Base {}

String tipo<T>(T x) => '$T';

void main() async {
  // E7: T fixed by the context of `print` (Object?).
  print(id(5) is int);
  print(nomeDe(id(5)));
  print(tipo(id(5)));
  // E7: list inferred in the context of `primeiro`.
  print(primeiro([7, 8]));
  // Context that flows down to the generic argument.
  print(tipo(first(singleton('x'))));
  print(tipo(maior(3, 9)));
  // E13: literals with the right context.
  final cs = [Caixa(1), Caixa('x')];
  print(cs.map((c) => c.tipo).toList());
  final m = [
    [1],
    [2, 3],
  ];
  print(m[0] is List<int>);
  final aninhada = [
    [1],
    [2, [3]],
  ];
  print(aninhada[0] is List<int>);
  print(aninhada[1] is List<int>);
  final mapa = {'a': 'b'};
  print(mapa is Map<String, String>);
  final regs = {(1, 2), (a: 1, b: 2)};
  print(regs is Set<Record>);
  print(regs is Set<(int, int)>);
  print(<Comparable>[3] is List<Comparable<dynamic>>);
  // E9 / E10: closures without `return` and that only throw.
  final f1 = Future(() {
    print('corpo');
  });
  print(f1 is Future<Null>);
  await f1;
  final f2 = Future.microtask(() => throw StateError('x'));
  print(f2 is Future<Never>);
  try {
    await f2;
  } catch (e) {
    print('pegou ${e.runtimeType}');
  }
  // Collection inside a generic class.
  final p = Pilha<int>()..push(1);
  print(p.itens is List<int>);
  // E3: special typing of int.
  print(tipo(15.clamp(0, 10)));
  print(tipo(-7.remainder(2)));
  print(tipo(math.max(1, 2)));
  // Promotion by assignment: the declared type stays `Base?`.
  Base? b;
  b = Child();
  print(tipo(b));
  // List.generate: the closure parameter is int.
  final g = List.generate(3, (i) => tipo(i));
  print(g);
  // Generic call with a closure whose return fixes T.
  final r = [1, 2, 3].map((x) => x * 2.5);
  print(r is Iterable<double>);
  print([1, 2].expand((x) => [x, -x]).toList() is List<int>);
  print(<int>[3, 1, 2].fold(0, (a, x) => a + x) is int);
  print(tipo([1, 2].fold(0, (a, x) => a + x)));
}
