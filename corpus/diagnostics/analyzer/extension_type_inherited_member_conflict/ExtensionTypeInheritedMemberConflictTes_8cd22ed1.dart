class A {
  void foo() {}
}

extension type B(A it) {
  void foo() {}
}

extension type C(A it) implements A, B {
  void foo() {}
}
