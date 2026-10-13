class A {
  const A();
}
mixin M {}
mixin N {
  int get g => 0;
  static int s = 0;
}
mixin F {
  final int x = 0;
}
class B = A with M;
class D = A with N;
class E = A with F;
var b = const B();
var d = const D();
var e = const E();
