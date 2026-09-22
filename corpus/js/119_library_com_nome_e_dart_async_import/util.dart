// Biblioteca util com nome declarado; importada duas vezes pelo main com prefixos diferentes.
library meu.util;

import 'dart:math' as m;

var contador = 0;

int proximo() => ++contador;

int maximo(List<int> xs) => xs.fold(xs.first, (a, b) => m.max(a, b));

class Ponto {
  final int x, y;
  const Ponto(this.x, this.y);
  double distancia() => m.sqrt((x * x + y * y).toDouble());
  @override
  String toString() => '($x, $y)';
}

const origem = Ponto(0, 0);
