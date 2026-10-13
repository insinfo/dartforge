// R-MEM-01: membros da interface: classe, superclasse, mixin, interfaces;
// assinatura combinada de interfaces diferentes.
mixin M {
  String mm() => '';
}

class Base {
  num valor() => 1;
}

class D extends Base with M {
  @override
  int valor() => 2;
}

abstract class I1 {
  num get v;
}

abstract class I2 {
  int get v;
}

abstract class J implements I1, I2 {}

void f(D d, J j) {
  print([/*@*/d.valor(), /*@*/d.mm(), /*@*/j.v, /*@*/d.hashCode]);
}

void main() {}
