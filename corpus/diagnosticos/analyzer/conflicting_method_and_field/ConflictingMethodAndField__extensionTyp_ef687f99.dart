extension type A(int it) {
  external int foo;
}

extension type B(int it) implements A {
  void foo() {}
}
