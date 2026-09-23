// experimentos: macros
// Construtores completados por augmentation (como a saída do @JsonCodable:
// `external C.x(...)` e depois `augment C.x(...) : inicializadores`),
// redirecionamento completado, construtor novo e fábrica nova acrescentados
// (a classe deixa de ter o construtor padrão).
import augment 'construtores_aug.dart';

class Ponto {
  final int x;
  final int y;
  external Ponto(int x, int y);
  external Ponto.origem();
  @override
  String toString() => 'Ponto($x, $y)';
}

class Registro {
  final String nome;
  final int nivel;
}

void main() {
  print(Ponto(1, 2));
  print(Ponto.origem());
  print(Ponto.diagonal(4));
  var r = Registro.novo('ana');
  print('${r.nome} ${r.nivel}');
  print(Registro.chefe('bia').nivel);
}
