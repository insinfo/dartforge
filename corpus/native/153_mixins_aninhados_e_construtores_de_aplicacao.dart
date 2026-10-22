// @dart=2.19
// Um mixin que é ele próprio uma aplicação de mixin (`class A = Object with M`
// aplicado a outra classe) traz os membros e os campos de `M` (linearização,
// tabela de métodos e mundo fechado); uma aplicação de mixin com nome
// (`class Application = Base with Mixin`) tem todos os construtores
// generativos da superclasse, nomeados inclusive.
class M<T> {
  int campo = 7;
  t() => T;
}

class A<U> = Object with M<List<U>>;

class B0 = Object with A<Set<bool>>;

class C0 extends B0 {}

class N {
  q() => 42;
}

class O<U> = Object with N;

class P<K, V> = Object with O<V>;

class Q<K, V> extends P<K, V> {}

mixin Mixin {
  int get x;
  int get m => x;
}

class Base {
  final int x;
  Base.c1(this.x);
  Base.c2([this.x = 37]);
  const Base.c4(this.x);
}

class Application = Base with Mixin;

void main() {
  print(C0().t());
  print(C0().campo);
  print(Q<bool, int>().q());
  print(Application.c1(42).m);
  print(new Application.c2().m);
  print(const Application.c4(5).m);
}
