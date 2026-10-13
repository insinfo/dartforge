class A {
  static int foo = 0;
  int bar = 0;
}

extension E on A {
  int get foo => 0;
  static int get bar => 0;
}
