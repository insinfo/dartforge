class A {
  int get foo => 0;
}

class B extends A {
  void foo() {}
}

augment class B {}
