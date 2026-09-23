class A {
  set foo(int _) {}
}

enum E implements A {
  v;
  @override
  set foo(int _) {}
}
