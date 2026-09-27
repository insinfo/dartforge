// Argumentos de tipo reificados que vêm da inferência (docs/INFERENCIA-JS-ALINHAMENTO.md §4):
// a instanciação de chamadas genéricas, literais de coleção, construtores e
// closures é observável por `is`, `as`, `runtimeType` e pelo `T` impresso.
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
  // E7: T fixado pelo contexto de `print` (Object?).
  print(id(5) is int);
  print(nomeDe(id(5)));
  print(tipo(id(5)));
  // E7: lista inferida no contexto de `primeiro`.
  print(primeiro([7, 8]));
  // Contexto que desce para o argumento genérico.
  print(tipo(first(singleton('x'))));
  print(tipo(maior(3, 9)));
  // E13: literais com o contexto certo.
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
  // E9 / E10: closures sem `return` e que só lançam.
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
  // Coleção dentro de classe genérica.
  final p = Pilha<int>()..push(1);
  print(p.itens is List<int>);
  // E3: tipagem especial de int.
  print(tipo(15.clamp(0, 10)));
  print(tipo(-7.remainder(2)));
  print(tipo(math.max(1, 2)));
  // Promoção por atribuição: o tipo declarado continua `Base?`.
  Base? b;
  b = Child();
  print(tipo(b));
  // List.generate: o parâmetro da closure é int.
  final g = List.generate(3, (i) => tipo(i));
  print(g);
  // Chamada genérica com closure cujo retorno fixa T.
  final r = [1, 2, 3].map((x) => x * 2.5);
  print(r is Iterable<double>);
  print([1, 2].expand((x) => [x, -x]).toList() is List<int>);
  print(<int>[3, 1, 2].fold(0, (a, x) => a + x) is int);
  print(tipo([1, 2].fold(0, (a, x) => a + x)));
}
