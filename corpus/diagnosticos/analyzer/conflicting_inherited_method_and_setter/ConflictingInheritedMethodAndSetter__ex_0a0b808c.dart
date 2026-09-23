extension type A(Object? it) {
  int get foo => 0;
}

extension type B(Object? it) {
  set foo(int _) {}
}

extension type C(Object? it) implements A, B {}
