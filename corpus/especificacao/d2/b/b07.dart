class S {
  S();
}
class M1 {}
class A = S with M1;
class B extends A {
  const B();
}
class Sem {}
class C extends Sem {
  const C();
}
class D extends Indef {
  const D();
}
extension type const ET(int i) {
  const ET.n() : i = 0;
}
