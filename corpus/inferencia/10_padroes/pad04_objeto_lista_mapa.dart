// R-PAD-04: padrões de objeto, lista (com resto) e mapa.
class Ponto {
  final int x;
  final double y;
  Ponto(this.x, this.y);
}

void f(Object o, List<int> l, Map<String, num> m) {
  if (o case Ponto(x: var px, :var y)) print([/*@*/px, /*@*/y]);
  if (l case [var h, ...var t]) print([/*@*/h, /*@*/t]);
  if (m case {'a': var a}) print(/*@*/a);
  if (o case List<int> li) print(/*@*/li);
  if (o case [var e]) print(/*@*/e);
}

void main() => f(1, [], {});
