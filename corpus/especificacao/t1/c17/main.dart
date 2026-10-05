part "p.dart";
class A {
  void foo() {}
}
void f(A a, B b) {
  a.foo();
  a.bar();
  b.foo();
  b.bar();
}
mixin B {
  void bar() {}
}
