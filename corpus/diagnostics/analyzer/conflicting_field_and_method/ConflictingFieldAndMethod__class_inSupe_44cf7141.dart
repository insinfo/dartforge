class A {
  void foo() {}
}

class B {
  int get foo => 0;
}

augment class B extends A {}
