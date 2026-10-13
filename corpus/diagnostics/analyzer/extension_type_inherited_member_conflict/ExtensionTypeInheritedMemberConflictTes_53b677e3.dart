extension type A1(int it) {
  void foo() {}
}

extension type A2(int it) {
  void foo() {}
}

extension type B(int it) implements A1, A2 {
  void foo() {}
}
