class A {
  void foo() {}
}
augment class A {
  void bar() {}
}
void f(A a) {
  a.foo();
  a.bar();
}
