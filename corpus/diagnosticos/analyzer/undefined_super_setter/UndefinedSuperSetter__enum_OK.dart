mixin M {
  set foo(int _) {}
}

enum E with M {
  v;
  void f() {
    super.foo = 0;
  }
}
