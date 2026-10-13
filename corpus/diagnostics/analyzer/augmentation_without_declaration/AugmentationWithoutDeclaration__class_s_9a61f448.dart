class A {
  static int get foo => 0;
}

augment class A {
  augment static abstract final int foo;
}
