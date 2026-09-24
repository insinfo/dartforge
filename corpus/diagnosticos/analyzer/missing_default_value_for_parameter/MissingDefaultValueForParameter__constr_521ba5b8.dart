extension type const E(int it) {}

class A {
  A({E a = const E(0)});
}

class B extends A {
  B({super.a});
}
