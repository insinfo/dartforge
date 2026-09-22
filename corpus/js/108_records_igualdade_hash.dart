// Igualdade e hashCode de records: posicional vs nomeado, ordem de nomeados, Set/Map, nulls, tipos diferentes.
class Ponto {
  final int x;
  final int y;
  Ponto(this.x, this.y);
  @override
  bool operator ==(Object o) => o is Ponto && o.x == x && o.y == y;
  @override
  int get hashCode => Object.hash(x, y);
  @override
  String toString() => 'P($x,$y)';
}

class SemEq {
  final int v;
  SemEq(this.v);
}

void main() {
  print((1, 2) == (1, 2));
  print((1, 2) == (2, 1));
  print((1, 2) == (1, 2, 3));
  print((a: 1, b: 2) == (a: 1, b: 2));
  print((a: 1, b: 2) == (b: 2, a: 1));
  print((a: 1, b: 2) == (a: 2, b: 1));
  print((1, b: 2) == (1, b: 2));
  print((1, 2) == (a: 1, b: 2));
  print((1, b: 2) == (b: 2, a: 1));
  print((x: 1) == (y: 1));

  print((1, 2).hashCode == (1, 2).hashCode);
  print((a: 1, b: 2).hashCode == (b: 2, a: 1).hashCode);
  final r1 = (1, 'x');
  final r2 = (1, 'x');
  print(r1 == r2);
  print(r1.hashCode == r2.hashCode);
  print(identical(r1, r2));

  print((1, 'a') == (1, 'b'));
  print((1, 'a') == ('a', 1));
  print((1, 2) == ('1', 2));
  print(((1, 2) as Object) == ((1, 2) as Object));
  final Object o1 = (1, 2);
  final Object o2 = (x: 1, y: 2);
  print(o1 == o2);
  print(o1 == (1, 2));

  print((null, 1) == (null, 1));
  print((null, 1) == (1, null));
  print((null, null) == (null, null));
  print((a: null) == (a: null));
  int? nulo;
  print((nulo, 2) == (null, 2));
  print((nulo,) == (0,));

  print((Ponto(1, 2), 3) == (Ponto(1, 2), 3));
  print((Ponto(1, 2), 3) == (Ponto(2, 1), 3));
  print((p: Ponto(0, 0)) == (p: Ponto(0, 0)));
  print((Ponto(1, 2), 3).hashCode == (Ponto(1, 2), 3).hashCode);
  final s1 = SemEq(1);
  print((s1, 1) == (s1, 1));
  print((SemEq(1), 1) == (SemEq(1), 1));
  print(([1, 2], 3) == ([1, 2], 3));
  final lista = [1, 2];
  print((lista, 3) == (lista, 3));
  print(((1, 2), 3) == ((1, 2), 3));
  print(((1, 2), 3) == ((1, 3), 3));
  print(('a', (b: 'b', c: ('c',))) == ('a', (b: 'b', c: ('c',))));

  final conjunto = {(1, 2), (1, 2), (2, 1), (a: 1, b: 2), (b: 2, a: 1)};
  print(conjunto.length);
  print(conjunto.contains((2, 1)));
  print(conjunto.contains((a: 1, b: 2)));
  print(conjunto.contains((1, 2, 3)));
  print(conjunto.contains((b: 1, a: 2)));

  final mapa = <(String, int), String>{};
  mapa[('a', 1)] = 'primeiro';
  mapa[('a', 1)] = 'sobrescrito';
  mapa[('a', 2)] = 'segundo';
  mapa[('b', 1)] = 'terceiro';
  print(mapa.length);
  print(mapa[('a', 1)]);
  print(mapa[('a', 2)]);
  print(mapa[('c', 1)]);
  print(mapa.containsKey(('b', 1)));
  print(mapa.keys.toList());

  final nomeado = <({int x, int y}), int>{};
  nomeado[(x: 1, y: 2)] = 1;
  nomeado[(y: 2, x: 1)] = 2;
  print(nomeado);
  print(nomeado.length);

  final pontos = [(0, 0), (1, 1), (0, 0), (1, 1), (2, 2)];
  print(pontos.toSet().length);
  print(pontos.indexOf((1, 1)));
  print(pontos.lastIndexOf((1, 1)));
  print(pontos.contains((2, 2)));
  print(pontos.where((p) => p == (0, 0)).length);
  final contagem = <(int, int), int>{};
  for (final p in pontos) {
    contagem[p] = (contagem[p] ?? 0) + 1;
  }
  print(contagem);
}
