mixin M {
  int get i => 0;
}
class A {
  const A();
}
class B = A with M;
var b = const B();
