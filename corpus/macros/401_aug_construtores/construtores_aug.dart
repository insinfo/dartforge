augment library 'main.dart';

augment class Ponto {
  augment Ponto(int x, int y)
      : this.x = x,
        this.y = y;
  augment Ponto.origem() : this(0, 0);
  Ponto.diagonal(int v) : this(v, v);
}

augment class Registro {
  Registro.novo(String nome) : this.nome = nome, this.nivel = 1;
  factory Registro.chefe(String nome) => Registro._(nome, 9);
  Registro._(this.nome, this.nivel) {
    print('criado $nome');
  }
}
