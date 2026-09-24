class A {
  void foo() {}
}

mixin B on A {}

augment mixin B {
  int get foo => 0;
}
