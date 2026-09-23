class A {
  static int get foo => 0;
  int get bar => 0;
}

extension E on A {
  int get foo => 0;
  static int get bar => 0;
}
