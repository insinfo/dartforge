extension type A(int it) {
  void foo() {}
}

extension type B(int it) implements A {
  set foo(int _) {}
}
