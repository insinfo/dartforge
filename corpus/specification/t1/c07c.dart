void f(A a) {
  a.foo();
  a.bar();
}
class A {
  void foo() {}
}
class A {
  void bar() {
    this.bar();
    this.foo();
  }
}
