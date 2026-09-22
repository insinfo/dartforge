// Extensions genéricas e estáticas: em record (T, T), Map<K, V>, membros estáticos, classe própria, em tipo função, Iterable<num> com sum, restrição de tipo.
extension Par<T> on (T, T) {
  (T, T) get trocado => ($2, $1);
  T get primeiro => $1;
  List<T> get lista => [$1, $2];
  bool get iguais => $1 == $2;
  (R, R) mapa<R>(R Function(T) f) => (f($1), f($2));
}

extension MapExt<K, V> on Map<K, V> {
  Map<V, K> get invertido => {for (final e in entries) e.value: e.key};
  List<String> get pares => [for (final e in entries) '${e.key}=${e.value}'];
  V ouPadrao(K k, V padrao) => containsKey(k) ? this[k] as V : padrao;
  Map<K, V> filtra(bool Function(K, V) f) =>
      {for (final e in entries) if (f(e.key, e.value)) e.key: e.value};
}

extension Soma on Iterable<num> {
  num get soma => fold(0, (a, b) => a + b);
  num get maximo => fold<num>(first, (a, b) => a > b ? a : b);
}

extension SomaInt on Iterable<int> {
  int get somaInt => fold(0, (a, b) => a + b);
  double get media => isEmpty ? 0 : somaInt / length;
}

extension Funcoes on int Function(int) {
  int Function(int) compoe(int Function(int) g) => (x) => this(g(x));
  int Function(int) get duasVezes => (x) => this(this(x));
  List<int> aplicaEm(List<int> xs) => xs.map(this).toList();
}

class Ponto {
  final int x, y;
  const Ponto(this.x, this.y);
  @override
  String toString() => '($x, $y)';
}

extension PontoExt on Ponto {
  static const origem = Ponto(0, 0);
  static Ponto de(List<int> xs) => Ponto(xs[0], xs[1]);
  static int criados = 0;
  Ponto operator +(Ponto o) => Ponto(x + o.x, y + o.y);
  Ponto get transposto => Ponto(y, x);
  int get manhattan => x.abs() + y.abs();
  Ponto escala(int f) {
    criados++;
    return Ponto(x * f, y * f);
  }
}

extension Util<T extends Comparable<Object>> on List<T> {
  T get maior => reduce((a, b) => a.compareTo(b) >= 0 ? a : b);
  List<T> get ordenada => [...this]..sort();
}

extension Nested<T> on List<List<T>> {
  List<T> get achatada => [for (final xs in this) ...xs];
  List<List<T>> get transposta => isEmpty
      ? []
      : [
          for (var i = 0; i < first.length; i++) [for (final l in this) l[i]]
        ];
}

void main() {
  final p = (1, 2);
  print(p.trocado);
  print(p.primeiro);
  print(p.lista);
  print(p.iguais);
  print((3, 3).iguais);
  print(('a', 'b').mapa((s) => s.toUpperCase()));
  print(('x', 'y').mapa((s) => s.length));
  print(Par<int>((5, 6)).trocado);
  print('--');
  final m = {'um': 1, 'dois': 2, 'tres': 3};
  print(m.invertido);
  print(m.pares);
  print(m.ouPadrao('dois', -1));
  print(m.ouPadrao('quatro', -1));
  print(m.filtra((k, v) => v.isOdd));
  print(<String, int>{}.pares);
  print('--');
  print([1, 2, 3].soma);
  print([1.5, 2.25].soma);
  print(<num>[1, 2.5].soma);
  print([4, 9, 2].maximo);
  print([1, 2, 3, 4].somaInt);
  print([1, 2, 4].media);
  print(<int>[].media.toStringAsFixed(1));
  print({1, 2, 3}.somaInt);
  print('--');
  int dobro(int x) => x * 2;
  int mais1(int x) => x + 1;
  print(dobro.compoe(mais1)(5));
  print(mais1.compoe(dobro)(5));
  print(dobro.duasVezes(3));
  print(dobro.aplicaEm([1, 2, 3]));
  print(((int x) => x - 1).duasVezes(10));
  print('--');
  print(PontoExt.origem);
  print(PontoExt.de([3, 4]));
  print(Ponto(1, 2) + Ponto(10, 20));
  print(Ponto(1, 2).transposto);
  print(Ponto(-3, 4).manhattan);
  Ponto(1, 1).escala(2);
  Ponto(1, 1).escala(3);
  print('criados: ${PontoExt.criados}');
  print(Ponto(2, 3).escala(4));
  print('--');
  print([3, 1, 2].maior);
  print(['pera', 'uva', 'kiwi'].maior);
  print([3, 1, 2].ordenada);
  print(['pera', 'uva', 'kiwi'].ordenada);
  print([[1, 2], [3], [4, 5]].achatada);
  print([[1, 2, 3], [4, 5, 6]].transposta);
  print(<List<int>>[].transposta);
  print('fim');
}
