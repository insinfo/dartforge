class A {
  const A();
}
class B extends A {
  const B();
}
class C {
  final A a;
  const C(this.a);
}
var v = const C(const B());
