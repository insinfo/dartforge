// Casos do mundo fechado (crates/mundo/tests/mundo.rs).

abstract class I {
  void f();
}

class SoTipo {}

class A implements I {
  void f() => print('A.f');
  void g() => print('A.g');
  void morto() {}
}

class B {
  void f() => print('B.f');
}

class NaoInstanciada {
  void f() {}
}

class Comp implements Comparable<Comp> {
  final int v;
  Comp(this.v);
  int compareTo(Comp o) => v - o.v;
  String toString() => 'C$v';
  void naoUsado() {}
}

class Base {
  void s() => print('Base.s');
}

class Deriv extends Base {
  void s() {
    super.s();
  }
}

mixin M {
  void doMixin() => print('M');
  void mixinMorto() {}
}

class ComMixin with M {}

class Nsm {
  noSuchMethod(Invocation i) => 'nsm';
}

class Estat {
  static int contador = 0;
  static void usado() {}
  static void naoUsado() {}
}

enum Cor { verde, azul }

class Json {
  Map toJson() => {};
}

int topoUsado() => 1;
int topoMorto() => 2;
var preguicoso = topoMorto2();
int topoMorto2() => 3;

extension E on String {
  String grita() => toUpperCase();
  String cala() => this;
}

class Fab {
  Fab._();
  factory Fab() = Fab._;
}

class Tear {
  Tear();
}

// Espécie de seletor: leitura (`v`) e escrita (`v=`) são independentes.
class SoLeitura {
  int _v = 0;
  int get v => _v;
  set v(int x) {
    _v = x;
  }
}

class SoEscrita {
  int _w = 0;
  int get w => _w;
  set w(int x) {
    _w = x;
  }
}

class ConstUsada {
  const ConstUsada();
}

class ConstMorta {
  const ConstMorta();
}

const usada = ConstUsada();
const morta = ConstMorta();

void main() {
  dynamic d = A();
  d.f();
  B().f();
  Object o = 1;
  print(o is SoTipo);
  var l = [Comp(2), Comp(1)]..sort();
  print(l);
  Deriv().s();
  ComMixin().doMixin();
  print(Nsm());
  Estat.usado();
  Estat.contador++;
  print(Cor.verde);
  print(topoUsado());
  print('x'.grita());
  Fab();
  var t = Tear.new;
  t();
  print(usada);
  var sl = SoLeitura();
  print(sl.v);
  var se = SoEscrita();
  se.w = 1;
  print(se._w);
}
