mixin M1 {
  int get foo => 0;
}

mixin M2 on M1 {
  void bar() {
    super.foo;
  }
}

enum E with M1, M2 {
  v
}
