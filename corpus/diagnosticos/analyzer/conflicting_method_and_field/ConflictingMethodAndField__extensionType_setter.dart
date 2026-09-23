extension type A(int it) {
  set foo(int _) {}
}

extension type B(int it) implements A {
  void foo() {}
}
