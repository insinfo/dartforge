class A {
  void staticMethod() {}
}

void f(A a) {
  a.staticMethod.call;
}
