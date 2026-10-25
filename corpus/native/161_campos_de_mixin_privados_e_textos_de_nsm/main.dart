// Os campos de uma `mixin class` (ou de uma classe aplicada como mixin) ficam
// na posição da classe que a aplica, inclusive com superclasse com campos e
// construtor nomeado; um membro privado homônimo declarado em outra biblioteca
// não sobrescreve o da biblioteca original (despacho estático, dinâmico e
// tear-off); o texto do `NoSuchMethodError` de uma chamada dinâmica sem membro
// é o da VM (`Class 'C' has no instance getter 'x'.` / `Receiver:` /
// `Tried calling:`), com os argumentos de tipo pelo nome do tipo.
import 'lib.dart';

class Base {
  int i, j;
  Base.ctor(int i, this.j) : i = i + 7;
}

abstract mixin class Counter {
  int get i;
  int k = 42;
  int twice() => k * 2;
  void bump() {
    k++;
  }
}

class C extends Base with Counter {
  int l = 131;
  C() : super.ctor(1, 13);
}

class Plain with Counter {
  int get i => 0;
}

class D with M {
  int _p() => 0;
}

class B extends A {
  int _q() => 0;
}

class Missing {
  int foo() => 1;
}

void main() {
  final c = C();
  c.bump();
  print([c.i, c.j, c.k, c.l, c.twice()]);
  Counter asCounter = c;
  asCounter.k = 7;
  print([asCounter.k, c.twice(), c.i, c.l]);
  final p = Plain()..bump();
  print([p.k, p.twice()]);

  print([callP(D()), D()._p(), callQ(B()), B().callQ(), B()._q()]);
  dynamic d = D();
  print([d._p(), dynamicP(D()), tearP(D())]);

  dynamic m = Missing();
  for (final f in [
    () => m.bar,
    () => m.bar = 1,
    () => m.baz(1, x: 2),
    () => m.qux<int, String>(),
  ]) {
    try {
      f();
    } on NoSuchMethodError catch (e) {
      print(e);
    }
  }
}
