class A {
  static abstract int foo;
  augment static int get foo => 0;
  augment static set foo(int _) {}
}
