class A {
  void foo(num a) {}
}

class B extends A {}

augment class B {
  void foo(covariant int a) {}
}
