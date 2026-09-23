class B {
  final A a;
  const B() : a = const A();
//      ^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
}
class A {
  final B b;
  const A() : b = const B();
//      ^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
}
