class A {
  int get foo => 0;
}

enum E implements A {
  v;
  @override
  int get foo => 0;
}
