class A {
  A();
  A.n();
  const A.c();
}
class B extends A {
  const B();
}
class C extends A {
  const C() : super();
  const C.n() : super.n();
  const C.c() : super.c();
  const C.a(int x) : assert(x > 0), super.n();
}
class D extends A {
  const D.nome();
}
