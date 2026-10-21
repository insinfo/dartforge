// Atribuição composta, `++`/`--` e `??=` num getter/setter e nos operadores de
// índice de uma extensão: o mundo fechado mantém o par (o alvo resolve só um
// deles; com outra extensão de getter homônimo, o getter era podado e a
// leitura lançava).
class Caixa {
  int v = 1;
  int? talvez;
}

extension Acesso on Caixa {
  int get dobro => v * 2;
  set dobro(int x) => v = x ~/ 2;
  int? get opcional => talvez;
  set opcional(int? x) => talvez = x;
  int operator [](int i) => v + i;
  operator []=(int i, int x) => v = x - i;
}

extension on int {
  int get dobro => this * 2;
}

void main() {
  print(21.dobro);
  final c = Caixa();
  c.dobro += 4;
  print(c.v);
  c.dobro++;
  print(c.v);
  --c.dobro;
  print(c.v);
  print(c.opcional ??= 7);
  print(c.opcional ??= 8);
  c[1] += 5;
  print(c.v);
  print(c[2]++);
  print(c.v);
}
