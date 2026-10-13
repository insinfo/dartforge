class A {
  void foo() {}
}
class A {
  int y = 0;
  void bar() {
    bar();
    y;
  }
}
void f(A a) {
  a.foo();
  a.bar();
}
