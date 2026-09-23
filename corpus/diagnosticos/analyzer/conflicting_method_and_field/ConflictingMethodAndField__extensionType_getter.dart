extension type A(int it) {
  int get foo => 0;
}

extension type B(int it) implements A {
  void foo() {}
}
