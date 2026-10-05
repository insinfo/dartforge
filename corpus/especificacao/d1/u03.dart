class B {
  final A a;
  const B() : a = const A();
}
class A {
  final B b;
  const A() : b = const B();
}
class Z {
  final Z z;
  const Z() : z = const Z();
}
const k = const A();
