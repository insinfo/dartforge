class A {
  void foo() {}
}

mixin B {
  int get foo => 0;
}

augment mixin B on A {}
