extension type A(int it) {
  void set foo(String _) {}
}

extension type B(int it) implements A {
  int get foo => 0;
}
