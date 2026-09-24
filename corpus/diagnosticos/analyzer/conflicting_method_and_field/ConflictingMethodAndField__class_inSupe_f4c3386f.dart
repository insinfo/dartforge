class A {
  int get foo => 0;
}

class B extends A {}

augment class B {
  void foo() {}
}
