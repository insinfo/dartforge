abstract class A { A(); factory A.f() = B; const A.c(); }
class B extends A { B() : super(); }
mixin M {}
abstract mixin class AM {}
void f() {
  new A();
  A();
  A.f();
  const A.c();
  new A.nada();
  new M();
  M();
  M.named();
  const M();
  new AM();
}
