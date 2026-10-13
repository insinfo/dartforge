class A {}

extension type A1(A it) {
  void foo() {}
}

extension type B(A it) {
  set foo(int _) {}
}

extension type C(A it) implements A1, B {
  int get foo => 0;
}
