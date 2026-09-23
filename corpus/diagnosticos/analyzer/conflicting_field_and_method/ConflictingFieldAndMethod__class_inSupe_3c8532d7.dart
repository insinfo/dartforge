class A {
  void foo() {}
}

class B extends A {}

augment class B {
  int get foo => 0;
}
