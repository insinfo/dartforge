class A {
  int x = 0;
  const A();
  const A.nome();
  const factory A.f() = A;
  A.nc();
}
class S {
  int y = 0;
}
class B extends S {
  const B();
}
mixin M {
  int z = 0;
}
class C with M {
  const C();
}
class D {
  abstract int w;
  static int s = 0;
  final int f = 0;
  const D();
}
class E {
  late final int l;
  external int e;
  const E();
}
