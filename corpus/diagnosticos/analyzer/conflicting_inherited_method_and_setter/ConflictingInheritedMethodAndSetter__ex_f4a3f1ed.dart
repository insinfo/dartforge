class A {
  void foo() {}
}

extension type B(A it) {
  set foo(int _) {}
}

extension type C(A it) implements A, B {
  int get foo => 0;
}
