// funções genéricas: T id<T>, bounds, várias variáveis de tipo, inferência, argumento explícito, closures genéricas.
T id<T>(T x) => x;

T primeiro<T>(List<T> xs) => xs[0];

List<T> repete<T>(T x, int n) => List<T>.filled(n, x);

T maior<T extends Comparable<Object>>(T a, T b) => a.compareTo(b) >= 0 ? a : b;

num somaNum<T extends num>(List<T> xs) {
  num s = 0;
  for (final x in xs) {
    s += x;
  }
  return s;
}

Map<K, V> paresParaMapa<K, V>(List<(K, V)> pares) {
  final m = <K, V>{};
  for (final (k, v) in pares) {
    m[k] = v;
  }
  return m;
}

(B, A) troca<A, B>(A a, B b) => (b, a);

R aplica<T, R>(T x, R Function(T) f) => f(x);

List<R> mapeia<T, R>(List<T> xs, R Function(T) f) => [for (final x in xs) f(x)];

T Function() constante<T>(T v) => () => v;

T Function(T) compor<T>(T Function(T) f, T Function(T) g) => (x) => g(f(x));

String tipoDe<T>(T x) {
  if (x is int) return 'int';
  if (x is String) return 'String';
  if (x is List) return 'List';
  return 'outro';
}

bool ehInt<T>() => T == int;

class Caixa<T> {
  final T v;
  Caixa(this.v);
  Caixa<R> mapa<R>(R Function(T) f) => Caixa<R>(f(v));
  @override
  String toString() => 'Caixa($v)';
}

void main() {
  print(id(5));
  print(id('s'));
  print(id<double>(2.5));
  print(id<int?>(null));
  print(primeiro([7, 8]));
  print(primeiro(['a']));
  print(repete('x', 3));
  print(repete<int>(0, 2));
  print(maior(3, 9));
  print(maior('pera', 'abacate'));
  print(somaNum([1, 2, 3]));
  print(somaNum([0.5, 0.25]));
  print(paresParaMapa([('a', 1), ('b', 2)]));
  print(troca(1, 'um'));
  print(troca<String, bool>('s', true));
  print(aplica(4, (x) => x * x));
  print(aplica('abc', (s) => s.length));
  print(aplica<int, String>(3, (x) => 'n$x'));
  print(mapeia([1, 2, 3], (x) => x.isEven));
  print(mapeia(['a', 'bb'], (s) => s.length));

  // closure genérica retornada
  final c = constante(42);
  print(c());
  final cs = constante<String>('fixo');
  print(cs());

  // composição
  final inc = (int x) => x + 1;
  final dup = (int x) => x * 2;
  print(compor(inc, dup)(5));
  print(compor(dup, inc)(5));
  print(compor<String>((s) => '$s!', (s) => s.toUpperCase())('oi'));

  // inferência com is
  print(tipoDe(1));
  print(tipoDe('x'));
  print(tipoDe([1]));
  print(tipoDe(2.5));

  // argumento de tipo em tempo de execução
  print(ehInt<int>());
  print(ehInt<String>());

  // generic tearoff instanciado
  final int Function(int) idInt = id;
  print(idInt(9));
  final idStr = id<String>;
  print(idStr('t'));
  print([1, 2].map(id).toList());
  print(['a', 'b'].map(id<String>).toList());
  final List<int> Function(int, int) rep = repete;
  print(rep(1, 3));

  // método genérico em classe genérica
  final caixa = Caixa(10);
  print(caixa.mapa((v) => 'v=$v'));
  print(caixa.mapa((v) => v > 5).mapa((b) => b ? 'sim' : 'não'));

  // função genérica local
  List<T> par<T>(T a, T b) => [a, b];
  print(par(1, 2));
  print(par('x', 'y'));
  print(par<num>(1, 2.5));

  // tipo inferido de retorno em lista
  final lista = [id(1), id(2)];
  print(lista.length);
  print(lista is List<int>);
}
