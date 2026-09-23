mixin M {
  set(int i) {}
}
class A {
  const A();
}
class B = A with M;
var b = const B();
