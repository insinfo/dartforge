extension type A(int it) {
  void foo() {}
}

extension type B1(int it) implements A {}

extension type B2(int it) implements A {}

extension type C(int it) implements B1, B2 {}
